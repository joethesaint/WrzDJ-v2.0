"""Tests for kiosk-pair nonce mechanism."""

import time

from fastapi.testclient import TestClient


class TestPairChallenge:
    def test_returns_nonce_with_expiry(self, client: TestClient):
        response = client.get("/api/public/kiosk/pair-challenge")
        assert response.status_code == 200
        body = response.json()
        assert "nonce" in body
        assert isinstance(body["nonce"], str)
        # secrets.token_urlsafe(16) → ~22-char base64url string
        assert len(body["nonce"]) >= 16
        assert body["expires_in"] == 10

    def test_pair_with_valid_nonce_succeeds(self, client: TestClient):
        challenge = client.get("/api/public/kiosk/pair-challenge").json()
        response = client.post(
            "/api/public/kiosk/pair",
            headers={"X-Pair-Nonce": challenge["nonce"]},
        )
        assert response.status_code == 200
        assert "pair_code" in response.json()

    def test_pair_without_nonce_400(self, client: TestClient):
        response = client.post("/api/public/kiosk/pair")
        assert response.status_code == 400
        assert "nonce" in response.json()["detail"].lower()

    def test_pair_with_invalid_nonce_400(self, client: TestClient):
        client.get("/api/public/kiosk/pair-challenge")  # Issue one for this IP
        response = client.post(
            "/api/public/kiosk/pair",
            headers={"X-Pair-Nonce": "totally-wrong-nonce-value-here"},
        )
        assert response.status_code == 400

    def test_nonce_single_use(self, client: TestClient):
        challenge = client.get("/api/public/kiosk/pair-challenge").json()
        # First use succeeds
        first = client.post(
            "/api/public/kiosk/pair",
            headers={"X-Pair-Nonce": challenge["nonce"]},
        )
        assert first.status_code == 200
        # Second use with same nonce fails (consumed)
        second = client.post(
            "/api/public/kiosk/pair",
            headers={"X-Pair-Nonce": challenge["nonce"]},
        )
        assert second.status_code == 400

    def test_nonce_expires_after_10s(self, client: TestClient, monkeypatch):
        """Mock time so we don't actually wait 10s."""
        from app.api import kiosk

        challenge = client.get("/api/public/kiosk/pair-challenge").json()
        # Fast-forward time
        real_time = time.time
        monkeypatch.setattr(kiosk.time, "time", lambda: real_time() + 11)
        response = client.post(
            "/api/public/kiosk/pair",
            headers={"X-Pair-Nonce": challenge["nonce"]},
        )
        assert response.status_code == 400
        assert "expired" in response.json()["detail"].lower()


class TestPairChallengeIndependence:
    """Issue #713: concurrent challenges from one IP stay independent."""

    def test_second_challenge_from_same_ip_keeps_first_valid(self, client: TestClient):
        first = client.get("/api/public/kiosk/pair-challenge").json()
        client.get("/api/public/kiosk/pair-challenge")  # second kiosk behind the same NAT
        response = client.post(
            "/api/public/kiosk/pair",
            headers={"X-Pair-Nonce": first["nonce"]},
        )
        assert response.status_code == 200

    def test_invalid_attempt_does_not_consume_valid_challenge(self, client: TestClient):
        challenge = client.get("/api/public/kiosk/pair-challenge").json()
        bad = client.post(
            "/api/public/kiosk/pair",
            headers={"X-Pair-Nonce": "totally-wrong-nonce-value-here"},
        )
        assert bad.status_code == 400
        good = client.post(
            "/api/public/kiosk/pair",
            headers={"X-Pair-Nonce": challenge["nonce"]},
        )
        assert good.status_code == 200

    def test_nonce_rejected_from_a_different_ip(self, client: TestClient, monkeypatch):
        from app.api import kiosk

        monkeypatch.setattr(kiosk, "get_client_ip", lambda request: "203.0.113.1")
        challenge = client.get("/api/public/kiosk/pair-challenge").json()
        monkeypatch.setattr(kiosk, "get_client_ip", lambda request: "203.0.113.2")
        response = client.post(
            "/api/public/kiosk/pair",
            headers={"X-Pair-Nonce": challenge["nonce"]},
        )
        assert response.status_code == 400
