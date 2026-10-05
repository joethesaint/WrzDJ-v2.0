from datetime import datetime

from pydantic import BaseModel, Field


class VibeVoteRequest(BaseModel):
    score: int = Field(..., ge=1, le=5, description="Guest vibe rating, 1 (low) to 5 (high)")


class VibeVoteResponse(BaseModel):
    status: str
    vibe_score: int = Field(..., ge=0, le=100, description="Time-weighted 0..100 rolling score")
    vote_count: int = Field(..., ge=0, description="Votes in the rolling window")
    track_title: str | None = None
    artist: str | None = None
    created_at: datetime
