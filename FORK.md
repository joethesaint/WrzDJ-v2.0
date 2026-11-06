# WrzDJ v2.0: this fork

This is joethesaint's fork of [wrzonance/WrzDJ](https://github.com/wrzonance/WrzDJ).

**How work moves:** it is prepared and reviewed before it lands here, then proposed upstream as a pull request.
Nothing goes upstream without an Issue first. Features still in review stay out of this fork until they are ready
to propose. `main` here tracks upstream `main`; this file is the only addition.

*Updated 2026-11-06. Upstream `main` at `0bbcf40`.*

## 1. In upstream: merged from this fork

| PR | Merged | Change |
|---|---|---|
| [#938](https://github.com/wrzonance/WrzDJ/pull/938) | 2026-11-05 | SSE: the stream's event lookup runs off the event loop; DB pool sized |
| [#932](https://github.com/wrzonance/WrzDJ/pull/932) | 2026-11-05 | Guest cookie lifetime configurable, shorter default (#909) |
| [#908](https://github.com/wrzonance/WrzDJ/pull/908) | 2026-11-05 | Provider drift check: live answers vs the test fakes |
| [#906](https://github.com/wrzonance/WrzDJ/pull/906) | 2026-11-05 | Tests: frozen clock, data factories, a Postgres-only set |
| [#905](https://github.com/wrzonance/WrzDJ/pull/905) | 2026-11-05 | Tests: warnings per run from ~2,100 to 3 |
| [#907](https://github.com/wrzonance/WrzDJ/pull/907) | 2026-10-28 | Dashboard: Next stops writing `dashboard/AGENTS.md` |
| [#894](https://github.com/wrzonance/WrzDJ/pull/894) | 2026-10-26 | Tests: no network in the backend suite |
| [#893](https://github.com/wrzonance/WrzDJ/pull/893) | 2026-10-26 | Dashboard tests: non-DOM files run under node |
| [#892](https://github.com/wrzonance/WrzDJ/pull/892) | 2026-10-26 | Spotify: rejected-cookie callbacks never reach the code exchange |
| [#891](https://github.com/wrzonance/WrzDJ/pull/891) | 2026-10-26 | ListenBrainz: live popularity endpoint |
| [#890](https://github.com/wrzonance/WrzDJ/pull/890) | 2026-10-26 | CI fails when `server/openapi.json` drifts |
| [#714](https://github.com/wrzonance/WrzDJ/pull/714) | 2026-10-04 | Kiosk: pair challenges independent and single-use (#713) |

## 2. Proposed upstream: open

| Issue | PR (draft) | Change |
|---|---|---|
| [#961](https://github.com/wrzonance/WrzDJ/issues/961) | [#962](https://github.com/wrzonance/WrzDJ/pull/962) | A replay that fails partway no longer drops the buffered tracks (CI green) |
| [#963](https://github.com/wrzonance/WrzDJ/issues/963) | [#964](https://github.com/wrzonance/WrzDJ/pull/964) | Stopping the Bridge sends one clear, and the server archives a song once |

## 3. On this fork only

| Branch | What | State |
|---|---|---|
| `rust/bridge-core` | The Bridge rewritten in Rust (below) | Builds and tests on Windows, macOS and Linux ([workflow](.github/workflows/bridge-core.yml) on that branch) |
| `fix/bridge-replay-keeps-tracks` | Head of #962 | Open PR |
| `fix/bridge-single-clear` | Head of #964 | Open PR |
| `fix/sse-lookup-off-loop`, `fix/guest-cookie-lifetime`, `chore/provider-drift-check`, `chore/tests-clock-factories-postgres`, `chore/tests-quiet-warnings` | Heads of merged PRs | Merged; kept for reference |

## 4. The Rust Bridge vs the current Bridge

The current Bridge is TypeScript on Node (and Electron for the app). The Rust Bridge is on `rust/bridge-core`, in
`bridge-core/`. Every piece is checked against the TypeScript with differential tests: generated inputs go through
both, and the outputs must match.

**Measured**

| | Current Bridge | Rust Bridge |
|---|---|---|
| Download | 180 MB installer | 2.5 MB program (Windows) |
| Memory, Traktor, 4 min | 89.6 → 93.0 MB | 3.0 → 3.8 MB |
| Memory, Serato, 2 min | 86.9 → 92.2 MB | 3.7 → 3.9 MB |
| CPU time, Traktor, 4 min | 0.54 s | 0.04 s |
| CPU time, Serato, 2 min | 0.23 s | 0.01 s |

**DJ software**

| Source | Current | Rust | Proof |
|---|---|---|---|
| Traktor (broadcast) | ✅ | ✅ | 20,000 generated streams, identical output |
| Serato (session file) | ✅ | ✅ | 3,000 generated sessions, identical output |
| StageLinQ (Denon) | ✅ | not yet | |
| Pro DJ Link (Pioneer) | ✅ | not yet | |

**Core**

| Feature | Current | Rust | Notes |
|---|---|---|---|
| Which deck is live | ✅ | ✅ | 20,000 traces, 1,013,386 events identical |
| Server calls, retries, circuit breaker | ✅ | ✅ | 5,000 traces, 135,347 requests identical |
| Replay after downtime | drops tracks (#961) | ✅ fixed | TypeScript fix in #962 |
| Clear on shutdown | sent twice (#963) | ✅ once | TypeScript and server fix in #964 |
| 401 "session expired" | ✅ (app) | ✅ | |
| Dashboard commands (reset, reconnect, restart) | ✅ | ✅ | Reconnect backs off 2, 4, 8, 16, 30 s |

**What the desktop app adds**

| Feature | Current (Electron) | Rust | Notes |
|---|---|---|---|
| Sign-in, API key, event list | ✅ | ✅ | Same endpoints and messages |
| Event health check (30 s) | ✅ | ✅ | Failed checks are logged |
| Network (shared subnet) warning | ✅ | ✅ | |
| Settings file | ✅ | ✅ | Reads the app's existing `config.json` |
| Token storage | OS safeStorage | OS keychain | Real keychain tested on Windows and Linux |
| Error log | `bridge.log` | `errors.log` + `errors.jsonl` with a run id; panics recorded | |
| Window and screens | ✅ | not yet | |
| Installer and updates | ✅ | not yet | |
