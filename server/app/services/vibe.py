"""Vibe Meter: guest vibe votes and the rolling score."""

import math
from dataclasses import dataclass
from datetime import datetime, timedelta

from sqlalchemy.orm import Session

from app.core.time import utcnow
from app.models.event import Event
from app.models.now_playing import NowPlaying
from app.models.vibe_event import VibeEvent

ROLLING_WINDOW_SECONDS = 30
GUEST_COOLDOWN_SECONDS = 3


class NoSongPlaying(Exception):
    """A vote arrived while no track is playing and the event rejects such votes."""


class VibeCooldown(Exception):
    """The guest voted within the cooldown; ``retry_after`` is whole seconds."""

    def __init__(self, retry_after: int):
        super().__init__(retry_after)
        self.retry_after = retry_after


@dataclass
class VibeResult:
    vote: VibeEvent
    score: int
    vote_count: int


def weighted_score(votes: list[tuple[int, datetime]], now: datetime) -> int | None:
    """Time-weighted 0..100 score over the rolling window.

    Each vote weighs ``ROLLING_WINDOW_SECONDS - age``, so a fresh vote counts fully
    and fades linearly to nothing at the window edge. The 1..5 weighted mean maps
    to 0..100. Returns None when no vote carries weight.
    """
    total_weight = 0.0
    weighted_sum = 0.0
    for score, created_at in votes:
        age = (now - created_at).total_seconds()
        weight = max(0.0, ROLLING_WINDOW_SECONDS - age)
        total_weight += weight
        weighted_sum += weight * score
    if total_weight == 0:
        return None
    mean = weighted_sum / total_weight
    return max(0, min(100, round((mean - 1.0) / 4.0 * 100)))


def record_vibe_vote(db: Session, event: Event, guest_id: int, score: int) -> VibeResult:
    """Store a guest's vote and return the rolling score for the current track.

    Raises VibeCooldown when the guest voted too recently, and NoSongPlaying when
    nothing is playing and the event has not opted in to keeping such votes.
    """
    now = utcnow()

    last = (
        db.query(VibeEvent.created_at)
        .filter(VibeEvent.event_id == event.id, VibeEvent.guest_id == guest_id)
        .order_by(VibeEvent.created_at.desc())
        .first()
    )
    if last is not None:
        elapsed = (now - last.created_at).total_seconds()
        if elapsed < GUEST_COOLDOWN_SECONDS:
            raise VibeCooldown(max(1, math.ceil(GUEST_COOLDOWN_SECONDS - elapsed)))

    now_playing = db.query(NowPlaying).filter(NowPlaying.event_id == event.id).first()
    if now_playing is None and not event.vibe_keep_no_song:
        raise NoSongPlaying()

    vote = VibeEvent(
        event_id=event.id,
        guest_id=guest_id,
        score=score,
        track_title=now_playing.title if now_playing else None,
        artist=now_playing.artist if now_playing else None,
        track_started_at=now_playing.started_at if now_playing else None,
        created_at=now,
    )
    db.add(vote)
    db.commit()
    db.refresh(vote)

    # Only votes for this same play count, so the meter starts fresh on every
    # track change; "no song" votes are scored among themselves.
    window = db.query(VibeEvent.score, VibeEvent.created_at).filter(
        VibeEvent.event_id == event.id,
        VibeEvent.created_at >= now - timedelta(seconds=ROLLING_WINDOW_SECONDS),
    )
    if vote.track_started_at is None:
        window = window.filter(VibeEvent.track_started_at.is_(None))
    else:
        window = window.filter(VibeEvent.track_started_at == vote.track_started_at)
    rows = window.all()

    computed = weighted_score([(r.score, r.created_at) for r in rows], now)
    return VibeResult(
        vote=vote, score=computed if computed is not None else 0, vote_count=len(rows)
    )
