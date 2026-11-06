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
| `rust/bridge-core` | The Bridge rewritten in Rust | Builds and tests on Windows, macOS and Linux |
| `fix/bridge-replay-keeps-tracks` | Head of #962 | Open PR |
| `fix/bridge-single-clear` | Head of #964 | Open PR |

Branches of merged PRs are deleted once merged; GitHub restores any of them from its PR page.

## 4. The Rust Bridge

What it traces from the current Bridge, what it improves, and what is not built yet:
[`bridge-core/README.md` on `rust/bridge-core`](https://github.com/joethesaint/WrzDJ-v2.0/blob/rust/bridge-core/bridge-core/README.md#where-it-stands-2026-11-06).

In short: Traktor and Serato match the TypeScript on generated tests, with about 25 times less memory and a 2.5 MB
program instead of a 180 MB installer. StageLinQ, Pro DJ Link and the app window are not built yet.
