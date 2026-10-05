from datetime import datetime, timedelta
import logging
from sqlalchemy import func
from sqlalchemy.orm import Session

from app.core.time import utcnow
from app.models.now_playing import NowPlaying
from app.models.vibe_event import VibeEvent

logger = logging.getLogger(__name__)

# 30-second rolling window per locked council default (Q9)
ROLLING_WINDOW_SECONDS = 30


def record_vibe_vote(
    db: Session,
    event_id: int,
    guest_id: int | None,
    score: int,
) -> tuple[VibeEvent, int, int]:
    """Record an audience vibe vote and calculate rolling normalized score (0..100).
    
    Adheres to Council decisions:
    - Q6 (4A-2 + 4B-2): captures current now_playing snapshot (title, artist, started_at).
    - Rolling window: last 30 seconds of votes.
    - Normalized score: maps 1..5 average to 0..100.
    """
    now = utcnow()
    now_playing = db.query(NowPlaying).filter(NowPlaying.event_id == event_id).first()

    track_title = now_playing.title if now_playing else None
    artist = now_playing.artist if now_playing else None
    track_started_at = now_playing.started_at if now_playing else None

    # Insert vote
    vibe = VibeEvent(
        event_id=event_id,
        guest_id=guest_id,
        score=score,
        track_title=track_title,
        artist=artist,
        track_started_at=track_started_at,
        created_at=now,
    )
    db.add(vibe)
    db.commit()
    db.refresh(vibe)

    # Compute rolling window score
    window_start = now - timedelta(seconds=ROLLING_WINDOW_SECONDS)
    
    # Base query for window
    query = db.query(VibeEvent.score, VibeEvent.created_at).filter(
        VibeEvent.event_id == event_id,
        VibeEvent.created_at >= window_start,
    )

    # If currently playing a track with started_at, reset votes before track start
    if track_started_at:
        query = query.filter(VibeEvent.created_at >= track_started_at)

    recent_votes = query.all()
    count = len(recent_votes)
    
    if count > 0:
        total_weight = 0.0
        weighted_sum = 0.0
        for vote_score, vote_created_at in recent_votes:
            # age_in_seconds can be slightly negative due to DB vs app time variance, so max with 0
            age_in_seconds = max(0.0, (now - vote_created_at).total_seconds())
            weight = max(0.0, float(ROLLING_WINDOW_SECONDS) - age_in_seconds)
            weighted_sum += vote_score * weight
            total_weight += weight
        
        raw_avg = (weighted_sum / total_weight) if total_weight > 0 else float(score)
    else:
        raw_avg = float(score)

    # Map 1.0..5.0 to 0..100
    normalized_score = int(round((raw_avg - 1.0) / 4.0 * 100))
    normalized_score = max(0, min(100, normalized_score))

    return vibe, normalized_score, count
