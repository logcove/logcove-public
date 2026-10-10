# Implementation plan

Current release (2026-10-09): v0.3.2 is published with compaction-aware download
retries and matching Skill guidance. Local checks and all five hosted platform
jobs passed. All seven archives were downloaded anonymously and matched their
checksums; macOS ARM64 binary smoke checks passed. See [release status](releases.md).

Agreed on 2026-09-08. This document distinguishes planned capabilities from implemented and verified behavior; a plan is not a release announcement.

Previous status (2026-10-03): the public v0.3.1 CLI and Skill are released. All five
platform jobs and Skill/release checks passed; anonymous release downloads and
checksums were verified. See [release status](releases.md). Dated earlier steps
below describe implementation history, not uncompleted release prerequisites.

## Goal and repository boundary

Complete the following workflow with a local coding agent:

```text
Browser authorization -> Project discovery -> Parquet download
  -> local DuckDB analysis -> Vega-Lite specification
  -> save Chart definition -> calculate/view in the web application
```

This repository holds a single Rust CLI crate under `cli/`, an analysis Skill under `skills/logcove/`, public user documentation under `docs/`, and CI/release configuration under `.github/workflows/`. The license is Apache-2.0.

The CLI depends only on public HTTP APIs. It does not read service databases, call ingestion control endpoints, or depend on private application source code. Deployment details, credentials, actual user logs, and infrastructure experiments do not belong in this repository.

## Responsibilities

The CLI implements deterministic operations: authentication, Project/Key management, pagination, file downloading, Chart definitions. The Skill guides the agent in setting up sources, selecting data, inspecting schemas, executing DuckDB, writing SQL, producing Vega-Lite, and checking results.

There is no CLI query engine, `data describe` command, bundled DuckDB, LLM client, background daemon, or SDK/code generation layer. The agent uses an independently installed DuckDB environment. Remote agents and containers can use the same released CLI and Skill with a personal access token supplied through `LOGCOVE_TOKEN`; no desktop credential store is required in PAT mode.

## Command scope

| Command | Batch | Responsibility |
| --- | --- | --- |
| `config show` / `config set-api-url <origin>` | 1 | Configure the target API without storing credentials in configuration |
| `login [--no-browser]` | 1 | Authorize in the user's browser and save a signed Session |
| `whoami` / `logout` | 1 | Inspect the current identity / revoke this CLI session |
| `projects list` / `projects get <id>` | 1 | Discover readable active Projects / inspect Project metadata |
| `projects create/update/archive/restore` | Management | Manage sources and their write-key bindings using existing public APIs |
| `keys list/get/create/update/set-projects/revoke` | Management | Manage write keys; creation writes the secret to an explicit private file and returns its path |
| `data pull <project-id> --from <date> --to <date> --output <dir> [--concurrency <1-8>]` | 2 | Download Parquet for inclusive UTC ingestion dates |
| `charts list/get/create/update/delete` | 2 | Manage Chart definitions |
| `skills install --agent codex\|claude [--force]` | 3 | Install the Skill embedded in the CLI into the selected agent's user directory, without API access |

Historical state on 2026-09-10: management commands were implemented in source, not yet released. `projects list --status` supports active (default), archived and all. Key output files use exclusive creation and owner permissions; no plaintext key is returned. The only new direct dependency is Windows-only `windows-sys` (already present transitively), used for native file ACLs. No backend change, billing command, ingestion client, query engine or new authentication flow is included. Desktop distribution can remain deferred while CLI workflows are completed.

Normal command output is stable JSON on stdout. Login instructions, progress, and warnings go to stderr. Failures return a nonzero exit code. Tokens and signed download URLs are not normal output. SQL and Vega-Lite specifications are accepted through files in batch 2 to avoid shell escaping.

## Batch 1: authentication and discovery

Configuration precedence is `--api-url`, then `LOGCOVE_API_URL`, then the saved API origin, then `https://api.logcove.com`. The production fallback is part of CLI 0.3.0. `--config-dir` / `LOGCOVE_CONFIG_DIR` allow an isolated configuration directory; otherwise use the OS configuration directory plus `logcove`. Configuration contains only the API origin.

Only HTTPS origins and loopback HTTP development origins are accepted. URL credentials, paths other than `/`, query strings, and fragments are rejected. HTTP redirects are not followed with a Session attached.

Login reuses the device authorization protocol with `client_id=logcove-cli`. Open the service's `verification_uri_complete` without reconstructing it. Print the link and verification code for manual use, including SSH terminals. Poll according to the returned interval and expiry, increase the interval on `slow_down`, and stop on denial/expiry. This is distinct from the desktop application's deep-link callback. No new server authentication endpoints are required.

The credential is a signed Better Auth Bearer Session, not a JWT or Vector write
key. The current source stores it in `~/.logcove/auth.json`, keyed by canonical
API origin, using private file permissions, atomic replacement and a per-user
lock. Published v0.3.2 and earlier use `keyring`; the switch is not yet released.
See [file credentials](file-credentials.md) for the implementation and migration.
CLI and desktop Sessions are independent; configuration directories do not
change the shared per-user Session identity.

Persist changed `set-auth-token` headers. A 401 or logout removes only the
matching stored Session, preserving newer logins. Network/server/permission
failures retain the Session. Logout revokes the remote Session first. PAT mode
uses `LOGCOVE_TOKEN` without reading or writing the Session file; invalid or
empty environment tokens never fall back to a saved Session.

Project listing traverses all `/api/v1/projects` pages, including empty pages with a next cursor. It defaults to active sources; `--status archived` or `--status all` includes archived metadata, which does not authorize reading archived logs. Detail uses `/api/v1/projects/{id}` and may describe an owned archived Project. Project names and descriptions provide context; they do not establish the actual Parquet schema.

Batch 1 acceptance: unit/HTTP contract tests, local API validation, actual browser authorization, persistence across separate processes, and documented platform verification. CI checks macOS, Linux, and Windows; a CI definition is not evidence that all platforms have already passed.

## Batch 2: download and Chart workflow

Download listing and signed URL requests follow the existing pagination contract, including batches of at most 20 signed URLs and a 16 KiB request-body limit. Use blocking HTTP with shared storage connection pools per pull, isolated from authenticated API requests. A bounded window of standard Rust threads streams up to four files concurrently by default (`--concurrency 1-8`); completed downloads free slots for pending files. API requests and credential operations stay on the main thread, including near-expiry refresh and a single re-sign after a storage 403. Finish the current signing batch before requesting the next. On an unrecoverable error, stop scheduling and join started downloads before returning. Write temporary files before making completed downloads visible, and preserve listing order in filenames and the completed manifest.

Create a manifest containing the API environment, Project ID, requested date range, successful local paths, object keys, sizes, ETags, and completion time. Do not persist signed URLs. The agent uses the manifest's files rather than a glob that could include unrelated earlier downloads.

Dates select UTC ingestion partitions, not event timestamps. Apply event-time filters in SQL when needed; logs are not required to contain `event_time`.

For portable SQL, the Skill registers a DuckDB view named by the full Project ID for each downloaded source. Saved SQL references those views, not machine-specific absolute paths. This is a client naming convention, not a new Chart field or implicit authorization rule. `project_ids` remain tag-like metadata.

Charts now store definitions only. Definition updates respect revision conflicts. Source SQL declares every Project dependency and has no parameters; the application filters every source view by system `_created_time` before running the calculation. Vega-Lite uses `data: {"name":"result"}`. Web/desktop calculates the selected time range locally and caches aggregate rows in memory; there is no result upload, history, thumbnail, query_params, or scheduled refresh. This contract is part of CLI 0.3.0.

Batch 2 acceptance: real authorized Parquet downloads, known DuckDB aggregates, Chart definition creation/update and local time-range calculation, and successful web rendering. The test API's R2 listing binding and download-signing target must point to the same real storage; locally emulated R2 cannot validate real downloads.

## Batch 3: Skill, followed by distribution

Create one `skills/logcove/SKILL.md` with the core workflow. Put detailed CLI usage, DuckDB examples, and Vega-Lite conventions in `references/` for on-demand reading. Maintain a common core for Codex and Claude Code with separate installation instructions.

The Skill checks identity, discovers sources, scopes the question and time range, downloads files, inspects schemas/samples, computes and checks results, and creates a Chart when the user wants one saved. Service interactions use the CLI; prompts do not ask agents to assemble authentication headers or handle Session secrets.

Complete and commit the Skill before changing CLI/Skill CI/CD. This first step is instruction-only and adds no runtime dependencies or CLI commands. Validate examples and the local workflow, and keep source installation distinct from released distribution.

In the following step, distribute prebuilt binaries via GitHub Releases and include checksums, license, and matching CLI/Skill documentation. Start development on macOS while checking macOS, Linux, and Windows builds. Record actual supported release targets and tested runtimes. The 2026-09-09 release decision allows CLI binary distribution before the hosted test environment: users configure an existing deployment explicitly. A CLI release is not a hosted-service launch.

Distribution is split into two batches. Batch 1 implements shared CI, five native platform builds (macOS ARM64/x64, Linux ARM64/x64, Windows x64), matching CLI/Skill archives, checksums and tag-triggered Releases. Batch 2 uses Homebrew/WinGet for CLI installation and updates, adds a bundled Skill installer, and verifies installation in fresh environments. CLI and Skill share the Cargo version. See [release procedures](releases.md) for triggers, artifacts, runtime boundaries and publication prerequisites.

The 2026-09-09 distribution decision selects Homebrew and WinGet only, with no npm package. Version 0.2.0 adds an embedded Skill installer and generates the package-manager metadata from actual archive hashes. The repository became public on 2026-09-23, and anonymous downloads of all v0.3.1 release archives have been verified. Public tap creation and WinGet submission remain separate publication steps. No self-update daemon or extra CLI runtime dependency is introduced. See [package-manager distribution](package-managers.md).

## Production and local testing (2026-09-23)

Production is deployed at these public addresses:

| Purpose | Address |
| --- | --- |
| Website | `https://logcove.com` |
| Web application | `https://app.logcove.com` |
| User API | `https://api.logcove.com` |
| HTTP JSON log ingestion | `https://ingest.logcove.com/logs` |
| OTLP HTTP Protobuf log ingestion | `https://ingest.logcove.com/v1/logs` |

Ingestion uses Project ID and a bound write key; each Project accepts its selected protocol only. CLI management and data reads use a browser-authorized Bearer Session or a personal access token (`LOGCOVE_TOKEN`). Write keys do not grant management or read access. `https://docs.logcove.com` remains reserved for a future documentation site; current public guides are in this repository.

Only production and local testing are maintained. The test web application and API run locally, normally at `http://localhost:5173` and `http://localhost:8787`, with separate test resources and the configured test collector. There is no hosted test web/API environment; the former `app-test`, `api-test`, and `ingest-test` domain plan was cancelled. Do not use those addresses or infer a test ingestion URL from the production hostname.

CLI v0.3.0 uses `https://api.logcove.com` as the built-in fallback, after `--api-url`, `LOGCOVE_API_URL`, and the saved API origin. Upgrades preserve saved origins and origin-scoped credentials. Local development uses an explicit API override. Published v0.2.0 predates this runtime change and still requires an explicit API origin.

## Dependencies and maintenance

| Dependency | Reason and alternative |
| --- | --- |
| `clap` | Typed command parsing and help instead of handwritten argument parsing |
| `reqwest` with `rustls` | Blocking HTTPS and streaming downloads without system OpenSSL |
| `serde` / `serde_json` | Explicit API contracts and JSON serialization |
| Rust standard file APIs + existing `windows-sys` | Private Session files, cross-process locking and atomic replacement; replaces `keyring` in the unreleased source |
| `directories` | OS-specific configuration paths without hand-coded platform rules |
| `open` | Open the existing browser with platform handling; manual links remain available |
| `time` | Parse calendar dates and timezone-aware link timestamps instead of maintaining handwritten leap-year and timestamp parsing; one pinned dependency with Rust-version and timezone-format checks |

Commit the application lockfile. Main maintenance costs are dependency/security updates, cross-platform file permissions, and cross-platform binary validation. Do not add async task orchestration, terminal UI frameworks, or a cross-repository shared SDK for this scope.

## Progress

- Batch 1 is implemented, with macOS tests, real browser authorization, native credential persistence, and local Project API verification completed. Linux and Windows verification boundaries are recorded in [validation.md](validation.md).
- Batch 2 download/manifest and Chart commands are implemented. macOS and Linux ARM64 automated tests pass. Real R2 downloads, DuckDB verification of 100,000 synthetic events, Chart result operations, and rendering in the existing web UI have passed; see [validation.md](validation.md).
- Batch 3 Skill authoring is implemented in `skills/logcove`, with binary-package installation instructions in [skills.md](skills.md) and actual verification in [skill-validation.md](skill-validation.md). Distribution batch 1 CI/build/package/release automation is implemented. All five platform jobs and Skill checks passed on GitHub Actions at `90ccde0` on 2026-09-09, with all six downloaded archives and checksum generation verified; see [hosted validation](releases.md#hosted-ci-and-artifact-validation-2026-09-09). The first binary release was v0.1.0, using explicit API configuration. The current release is v0.3.2; hosted test deployment is no longer planned. Remaining package-manager publication and platform verification are tracked in [package-manager distribution](package-managers.md).
