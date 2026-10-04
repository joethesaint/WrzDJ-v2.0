# Kiosk pairing challenge correctness

Implement issue #713 while preserving PR #714's author changes. Each challenge is
independent even behind one IP, bound to its issuing IP, valid for ten seconds and
accepted at most once. Missing, incorrect, wrong-IP and expired attempts must not
consume a different valid challenge. Both kiosks must be able to finish pairing.

FastAPI runs synchronous routes in a thread pool, including a single-worker uvicorn
process (https://fastapi.tiangolo.com/async/). Protect cache issuance/pruning and the
whole validation/consumption transaction with one threading.Lock. Database work stays
outside the lock. Retain rate limits, response schemas and the documented requirement
for a shared store if deployment changes to multiple processes.

Public API tests must expose the current simultaneous replay race and prove the fix,
while using a mocked create_kiosk service boundary only to avoid concurrent SQLite
fixture writes. Existing expiry, replay and IP tests remain. Run full backend CI gates,
latest-head remote CI and exactly one final cross-provider review before merging.
