"""Public API endpoints for audience vibe meter rating."""

from fastapi import APIRouter, Depends, HTTPException, Path, Request
from sqlalchemy.orm import Session

from app.api.deps import get_db, require_verified_human_soft
from app.core.rate_limit import get_guest_id, limiter
from app.schemas.vibe import VibeVoteRequest, VibeVoteResponse
from app.services.event import EventLookupResult, get_event_by_public_code_with_status
from app.services.event_bus import get_event_bus
from app.services.vibe import record_vibe_vote

router = APIRouter()


@router.post("/{code}/vibe", response_model=VibeVoteResponse)
@limiter.limit("20/minute")
def vote_event_vibe(
    code: str = Path(..., min_length=4, max_length=10),
    vote_data: VibeVoteRequest = ...,
    request: Request = None,
    db: Session = Depends(get_db),
    _human: int | None = Depends(require_verified_human_soft),
) -> VibeVoteResponse:
    """Submit audience vibe rating (1..5).
    
    Reuses existing WrzDJ security gates:
    - requires soft verified human cookie (Turnstile) or DEV_AUTH_BYPASS.
    - anonymous guest identity resolved via get_guest_id().
    - broadcasts real-time vibe_updated payload over process-local event_bus.
    """
    event_obj, lookup_result = get_event_by_public_code_with_status(db, code)
    if lookup_result == EventLookupResult.NOT_FOUND:
        raise HTTPException(status_code=404, detail="Event not found")
    if lookup_result in (EventLookupResult.EXPIRED, EventLookupResult.ARCHIVED):
        raise HTTPException(status_code=410, detail="Event has ended or is archived")

    guest_id = get_guest_id(request, db)
    if guest_id is None:
        raise HTTPException(status_code=401, detail="Guest identity required")

    vibe_entry, normalized_score, vote_count = record_vibe_vote(
        db=db,
        event_id=event_obj.id,
        guest_id=guest_id,
        score=vote_data.score,
    )

    # Broadcast over SSE bus
    bus = get_event_bus()
    bus.publish(
        event_code=code.upper(),
        event_type="vibe_updated",
        data={
            "vibe_score": normalized_score,
            "vote_count": vote_count,
            "track_title": vibe_entry.track_title,
            "artist": vibe_entry.artist,
        },
    )

    return VibeVoteResponse(
        status="recorded",
        vibe_score=normalized_score,
        vote_count=vote_count,
        track_title=vibe_entry.track_title,
        artist=vibe_entry.artist,
        created_at=vibe_entry.created_at,
    )
