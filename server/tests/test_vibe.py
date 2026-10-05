"""Tests for the Vibe Meter vote endpoint and rolling score."""

from datetime import timedelta
from unittest.mock import patch

import pytest
from fastapi.testclient import TestClient
from sqlalchemy.orm import Session

from app.core.time import utcnow
from app.models.event import Event
from app.models.guest import Guest
from app.models.now_playing import NowPlaying
from app.models.vibe_event import VibeEvent
from app.services import vibe as vibe_service
from app.services.vibe import ROLLING_WINDOW_SECONDS, weighted_score


def _play(db: Session, event: Event, title: str = "Midnight City", artist: str = "M83"):
    db.query(NowPlaying).filter(NowPlaying.event_id == event.id).delete()
    now_playing = NowPlaying(event_id=event.id, title=title, artist=artist, started_at=utcnow())
    db.add(now_playing)
    db.commit()
    return now_playing


def _new_guest(db: Session, tag: str) -> Guest:
    guest = Guest(token=tag * 64, fingerprint_hash=f"fp_{tag}")
    db.add(guest)
    db.commit()
    db.refresh(guest)
    return guest


def _vote(client: TestClient, event: Event, guest: Guest, score: int):
    client.cookies.clear()
    client.cookies.set("wrzdj_guest", guest.token)
    return client.post(f"/api/public/events/{event.code}/vibe", json={"score": score})


class TestWeightedScore:
    def test_no_votes_is_none(self):
        assert weighted_score([], utcnow()) is None

    def test_single_fresh_vote_maps_to_0_100(self):
        now = utcnow()
        assert weighted_score([(5, now)], now) == 100
        assert weighted_score([(1, now)], now) == 0
        assert weighted_score([(3, now)], now) == 50

    def test_older_votes_weigh_less(self):
        now = utcnow()
        old_low = (1, now - timedelta(seconds=25))
        fresh_high = (5, now)
        # Plain average would be 50; recency pulls it towards the fresh 5.
        assert weighted_score([old_low, fresh_high], now) > 80

    def test_votes_past_the_window_carry_no_weight(self):
        now = utcnow()
        stale = (1, now - timedelta(seconds=ROLLING_WINDOW_SECONDS))
        assert weighted_score([stale], now) is None


class TestVibeVote:
    def test_score_5_on_fresh_track_is_100(
        self, client: TestClient, db: Session, test_event: Event, test_guest: Guest
    ):
        _play(db, test_event)
        r = _vote(client, test_event, test_guest, 5)
        assert r.status_code == 200
        body = r.json()
        assert body["vibe_score"] == 100
        assert body["vote_count"] == 1
        assert body["track_title"] == "Midnight City"

    def test_second_vote_within_cooldown_is_429(
        self, client: TestClient, db: Session, test_event: Event, test_guest: Guest
    ):
        _play(db, test_event)
        assert _vote(client, test_event, test_guest, 4).status_code == 200
        r = _vote(client, test_event, test_guest, 4)
        assert r.status_code == 429
        assert 1 <= int(r.headers["Retry-After"]) <= 3

    def test_cooldown_is_per_guest_not_per_ip(
        self, client: TestClient, db: Session, test_event: Event, test_guest: Guest
    ):
        """Guests behind one IP (venue Wi-Fi) do not block each other."""
        _play(db, test_event)
        other = _new_guest(db, "b")
        assert _vote(client, test_event, test_guest, 5).status_code == 200
        r = _vote(client, test_event, other, 1)
        assert r.status_code == 200
        assert r.json()["vote_count"] == 2

    def test_vote_after_cooldown_is_accepted(
        self, client: TestClient, db: Session, test_event: Event, test_guest: Guest
    ):
        _play(db, test_event)
        assert _vote(client, test_event, test_guest, 4).status_code == 200
        later = utcnow() + timedelta(seconds=4)
        with patch.object(vibe_service, "utcnow", return_value=later):
            assert _vote(client, test_event, test_guest, 4).status_code == 200

    def test_track_change_resets_the_score(
        self, client: TestClient, db: Session, test_event: Event, test_guest: Guest
    ):
        _play(db, test_event, title="Old Song")
        assert _vote(client, test_event, test_guest, 1).status_code == 200

        _play(db, test_event, title="New Song")
        other = _new_guest(db, "c")
        r = _vote(client, test_event, other, 5)
        assert r.status_code == 200
        assert r.json()["vibe_score"] == 100
        assert r.json()["vote_count"] == 1

    def test_no_song_is_rejected_by_default(
        self, client: TestClient, db: Session, test_event: Event, test_guest: Guest
    ):
        r = _vote(client, test_event, test_guest, 3)
        assert r.status_code == 409
        assert db.query(VibeEvent).count() == 0

    def test_no_song_kept_when_event_opts_in(
        self, client: TestClient, db: Session, test_event: Event, test_guest: Guest
    ):
        test_event.vibe_keep_no_song = True
        db.commit()
        r = _vote(client, test_event, test_guest, 3)
        assert r.status_code == 200
        assert r.json()["track_title"] is None
        stored = db.query(VibeEvent).one()
        assert stored.track_started_at is None

    @pytest.mark.parametrize("score", [0, 6])
    def test_score_out_of_range_is_422(
        self, client: TestClient, db: Session, test_event: Event, test_guest: Guest, score: int
    ):
        _play(db, test_event)
        assert _vote(client, test_event, test_guest, score).status_code == 422

    def test_unknown_event_is_404(self, client: TestClient, test_guest: Guest):
        client.cookies.set("wrzdj_guest", test_guest.token)
        r = client.post("/api/public/events/NOPE99/vibe", json={"score": 3})
        assert r.status_code == 404

    def test_ended_event_is_410(
        self, client: TestClient, db: Session, test_event: Event, test_guest: Guest
    ):
        test_event.archived_at = utcnow()
        db.commit()
        assert _vote(client, test_event, test_guest, 3).status_code == 410

    def test_missing_guest_cookie_is_rejected(
        self, client: TestClient, db: Session, test_event: Event
    ):
        _play(db, test_event)
        client.cookies.clear()
        r = client.post(f"/api/public/events/{test_event.code}/vibe", json={"score": 3})
        assert r.status_code in (401, 403)

    def test_vote_publishes_on_the_canonical_channel(
        self, client: TestClient, db: Session, test_event: Event, test_guest: Guest
    ):
        _play(db, test_event)
        with patch("app.api.vibe.publish_event") as publish:
            assert _vote(client, test_event, test_guest, 5).status_code == 200
        publish.assert_called_once()
        channel, event_type, data = publish.call_args.args
        assert channel == test_event.code
        assert event_type == "vibe_updated"
        assert data["vibe_score"] == 100
