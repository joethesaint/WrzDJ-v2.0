"""Add vibe_events table and events.vibe_keep_no_song.

Revision ID: 070
Revises: 069
"""

import sqlalchemy as sa

from alembic import op

revision = "070"
down_revision = "069"
branch_labels = None
depends_on = None


def upgrade() -> None:
    op.create_table(
        "vibe_events",
        sa.Column("id", sa.Integer(), primary_key=True),
        sa.Column(
            "event_id",
            sa.Integer(),
            sa.ForeignKey("events.id", ondelete="CASCADE"),
            nullable=False,
        ),
        sa.Column(
            "guest_id",
            sa.Integer(),
            sa.ForeignKey("guests.id", ondelete="SET NULL"),
            nullable=True,
        ),
        sa.Column("score", sa.SmallInteger(), nullable=False),
        sa.Column("track_title", sa.String(length=255), nullable=True),
        sa.Column("artist", sa.String(length=255), nullable=True),
        sa.Column("track_started_at", sa.DateTime(timezone=True), nullable=True),
        sa.Column("created_at", sa.DateTime(), nullable=False),
        sa.CheckConstraint("score >= 1 AND score <= 5", name="ck_vibe_events_score_range"),
    )
    op.create_index("ix_vibe_events_event_created", "vibe_events", ["event_id", "created_at"])
    op.create_index(
        "ix_vibe_events_guest_event_created",
        "vibe_events",
        ["guest_id", "event_id", "created_at"],
    )
    op.add_column(
        "events",
        sa.Column("vibe_keep_no_song", sa.Boolean(), nullable=False, server_default="0"),
    )


def downgrade() -> None:
    op.drop_column("events", "vibe_keep_no_song")
    op.drop_index("ix_vibe_events_guest_event_created", table_name="vibe_events")
    op.drop_index("ix_vibe_events_event_created", table_name="vibe_events")
    op.drop_table("vibe_events")
