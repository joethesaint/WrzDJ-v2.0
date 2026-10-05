"""Public endpoint for guest Vibe Meter votes."""

from fastapi import APIRouter, Depends, HTTPException, Path, Request
from sqlalchemy.orm import Session

from app.api.deps import get_db, require_verified_human_soft
from app.core.rate_limit import get_guest_id, limiter
from app.schemas.vibe import VibeVoteRequest, VibeVoteResponse
from app.services.event import EventLookupResult, get_event_by_public_code_with_status
from app.services.event_bus import publish_event
from app.services.vibe import NoSongPlaying, VibeCooldown, record_vibe_vote

router = APIRouter()


@router.post("/{code}/vibe", response_model=VibeVoteResponse)
@limiter.limit("120/minute")
def vote_event_vibe(
    request: Request,
    vote_data: VibeVoteRequest,
    code: str = Path(..., min_length=4, max_length=10),
    db: Session = Depends(get_db),
    _human: int | None = Depends(require_verified_human_soft),
) -> VibeVoteResponse:
    """Record a guest's 1..5 vibe vote and broadcast the new rolling score.

    The per-guest cooldown is the real limit. The per-IP limit is only a flood
    guard, set high because a venue's guests often share one public IP.
    """
    event, lookup = get_event_by_public_code_with_status(db, code)
    if lookup == EventLookupResult.NOT_FOUND:
        raise HTTPException(status_code=404, detail="Event not found")
    if lookup in (EventLookupResult.EXPIRED, EventLookupResult.ARCHIVED):
        raise HTTPException(status_code=410, detail="Event has ended")

    guest_id = get_guest_id(request, db)
    if guest_id is None:
        raise HTTPException(status_code=401, detail="Guest identity required")

    try:
        result = record_vibe_vote(db, event, guest_id, vote_data.score)
    except VibeCooldown as exc:
        raise HTTPException(
            status_code=429,
            detail="Vote cooldown",
            headers={"Retry-After": str(exc.retry_after)},
        ) from exc
    except NoSongPlaying as exc:
        raise HTTPException(status_code=409, detail="No song is playing") from exc

    publish_event(
        event.code,
        "vibe_updated",
        {
            "vibe_score": result.score,
            "vote_count": result.vote_count,
            "track_title": result.vote.track_title,
            "artist": result.vote.artist,
        },
    )

    return VibeVoteResponse(
        status="recorded",
        vibe_score=result.score,
        vote_count=result.vote_count,
        track_title=result.vote.track_title,
        artist=result.vote.artist,
        created_at=result.vote.created_at,
    )
