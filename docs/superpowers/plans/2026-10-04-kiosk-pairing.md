# Kiosk Pairing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Preserve independent challenges and guarantee single-use consumption under concurrent requests.
**Architecture:** One process-local lock protects all cache transactions; database creation runs after release.
**Tech Stack:** FastAPI synchronous routes, threading.Lock, pytest/TestClient.
**Spec:** docs/superpowers/specs/2026-10-04-kiosk-pairing.md

## Global Constraints

- Preserve author commits, ten-second expiry, IP binding, rate limits and API schemas.
- No database operations under the cache lock; no multi-process support implied.
- Keep the 85% backend coverage gate; user already authorized PR updates and merging.

## Review Focus

- Concurrent replay must yield exactly one successful pairing and one 400.
- Both outstanding challenges behind one IP must complete independently.
- Missing and incorrect nonce attempts must leave a valid challenge usable.
- An expired challenge must not consume a younger valid challenge.
- Wrong-IP rejection must leave the issuing IP's challenge usable.

### Task 1: Make challenge transactions atomic

**Files:** Modify server/app/api/kiosk.py and server/tests/test_kiosk_pair_nonce.py.
**Interfaces:** Consumes existing GET /api/public/kiosk/pair-challenge and POST
/api/public/kiosk/pair. Produces the same response schemas/statuses, with at most
one successful POST for a valid nonce even when requests overlap.

- [x] Step 1: Add public API concurrent replay test referencing df1b9790. Wrap the
  IP comparison with bounded thread events to overlap two validations; mock only
  create_kiosk and assert sorted statuses [200,400] and exactly one service call.
  Expand existing independence and IP tests, parameterize missing/incorrect nonce,
  and add the older-expired/younger-valid case.
- [x] Step 2: Run `server/.venv/bin/pytest server/tests/test_kiosk_pair_nonce.py --no-cov -q`.
  Expected: concurrent replay fails [200,200]; all other PR behaviors pass.
- [x] Step 3: Add a shared threading.Lock; guard prune+insert and lookup+validate+consume.
  Compute issuance time inside the lock. Keep create_kiosk outside it and update
  the single-worker comment to explain thread synchronization and process limits.
- [x] Step 4: Run focused tests, full backend pytest, Ruff/format, Bandit and PostgreSQL
  Alembic upgrade/check. Expected: all pass, coverage at least 85%.
- [ ] Step 5: Commit with Codex attribution and run task-done with the focused command.
  Push back to the existing fork PR when permitted, verify current-head CI, obtain
  one final Claude Opus5 high review, fix verified findings with regressions, then
  merge the exact tested head and verify issue #713 closure and main checks.
