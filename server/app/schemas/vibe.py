from datetime import datetime
from pydantic import BaseModel, Field


class VibeVoteRequest(BaseModel):
    score: int = Field(..., ge=1, le=5, description="Audience vibe score between 1 (low) and 5 (high)")


class VibeVoteResponse(BaseModel):
    status: str
    vibe_score: int = Field(..., ge=0, le=100, description="Normalized 0..100 rolling average")
    vote_count: int = Field(..., ge=0, description="Total active votes in rolling window")
    track_title: str | None = None
    artist: str | None = None
    created_at: datetime
