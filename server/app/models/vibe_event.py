from datetime import datetime

from sqlalchemy import DateTime, ForeignKey, Index, Integer, SmallInteger, String
from sqlalchemy.orm import Mapped, mapped_column, relationship

from app.core.time import utcnow
from app.models.base import Base


class VibeEvent(Base):
    """Audience vibe vote event.
    
    Adheres to Council safe default Q6 (4A-2 + 4B-2):
    - Stores snapshot of track title, artist, started_at at vote time.
    - Preserves upstream schema without altering now_playing or play_history.
    - Score is 1..5 corresponding to guest emoji energy rating.
    """
    __tablename__ = "vibe_events"
    __table_args__ = (
        Index("ix_vibe_events_event_created", "event_id", "created_at"),
        Index("ix_vibe_events_guest_event", "guest_id", "event_id", "created_at"),
    )

    id: Mapped[int] = mapped_column(primary_key=True)
    event_id: Mapped[int] = mapped_column(
        ForeignKey("events.id", ondelete="CASCADE"), index=True
    )
    guest_id: Mapped[int | None] = mapped_column(
        ForeignKey("guests.id", ondelete="SET NULL"), nullable=True, index=True
    )
    score: Mapped[int] = mapped_column(SmallInteger, nullable=False)  # 1..5

    # Snapshot metadata per Council Decision 4B-2
    track_title: Mapped[str | None] = mapped_column(String(255), nullable=True)
    artist: Mapped[str | None] = mapped_column(String(255), nullable=True)
    track_started_at: Mapped[datetime | None] = mapped_column(DateTime, nullable=True)

    created_at: Mapped[datetime] = mapped_column(DateTime, default=utcnow, index=True)

    event: Mapped["Event"] = relationship("Event")
    guest: Mapped["Guest"] = relationship("Guest")
