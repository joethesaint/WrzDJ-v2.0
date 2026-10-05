from datetime import datetime

from sqlalchemy import DateTime, ForeignKey, Index, SmallInteger, String
from sqlalchemy.orm import Mapped, mapped_column

from app.core.time import utcnow
from app.models.base import Base


class VibeEvent(Base):
    """One guest vibe vote (1..5) for an event.

    Each vote snapshots the track playing when it was cast (title, artist and
    the now-playing ``started_at``), so a play can be reconstructed later without
    changing ``now_playing`` or ``play_history``. All three are null for a vote
    kept while no track was playing.
    """

    __tablename__ = "vibe_events"
    __table_args__ = (
        Index("ix_vibe_events_event_created", "event_id", "created_at"),
        Index("ix_vibe_events_guest_event_created", "guest_id", "event_id", "created_at"),
    )

    id: Mapped[int] = mapped_column(primary_key=True)
    event_id: Mapped[int] = mapped_column(ForeignKey("events.id", ondelete="CASCADE"))
    guest_id: Mapped[int | None] = mapped_column(
        ForeignKey("guests.id", ondelete="SET NULL"), nullable=True
    )
    score: Mapped[int] = mapped_column(SmallInteger, nullable=False)

    track_title: Mapped[str | None] = mapped_column(String(255), nullable=True)
    artist: Mapped[str | None] = mapped_column(String(255), nullable=True)
    track_started_at: Mapped[datetime | None] = mapped_column(
        DateTime(timezone=True), nullable=True
    )

    created_at: Mapped[datetime] = mapped_column(DateTime, default=utcnow)
