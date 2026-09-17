# Implementation plan

Agreed on 2026-09-08. This document distinguishes planned capabilities from implemented and verified behavior; a plan is not a release announcement.

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

There is no CLI query engine, `data describe` command, bundled DuckDB, LLM client, background daemon, or SDK/code generation layer. The agent uses an independently installed DuckDB environment. A future container can install the released CLI and the same Skill; noninteractive service identity is a later design.

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

2026-09-10: Management commands are implemented in source, not released. `projects list --status` supports active (default), archived and all. Key output files use exclusive creation and owner permissions; no plaintext key is returned. The only new direct dependency is Windows-only `windows-sys` (already present transitively), used for native file ACLs. No backend change, billing command, ingestion client, query engine or new authentication flow is included. Desktop distribution can remain deferred while CLI workflows are completed.

Normal command output is stable JSON on stdout. Login instructions, progress, and warnings go to stderr. Failures return a nonzero exit code. Tokens and signed download URLs are not normal output. SQL and Vega-Lite specifications are accepted through files in batch 2 to avoid shell escaping.

## Batch 1: authentication and discovery

Configuration precedence is `--api-url`, then `LOGCOVE_API_URL`, then the saved API origin, then `https://api.logcove.com`. The production fallback is part of CLI 0.3.0. `--config-dir` / `LOGCOVE_CONFIG_DIR` allow an isolated configuration directory; otherwise use the OS configuration directory plus `logcove`. Configuration contains only the API origin.

Only HTTPS origins and loopback HTTP development origins are accepted. URL credentials, paths other than `/`, query strings, and fragments are rejected. HTTP redirects are not followed with a Session attached.

Login reuses the device authorization protocol with `client_id=logcove-cli`. Open the service's `verification_uri_complete` without reconstructing it. Print the link and verification code for manual use, including SSH terminals. Poll according to the returned interval and expiry, increase the interval on `slow_down`, and stop on denial/expiry. This is distinct from the desktop application's deep-link callback. No new server authentication endpoints are required.

The credential is a signed Better Auth Bearer Session, not a JWT and not a Vector write key. Store it through `keyring` with service `com.logcove.cli.session`, keyed by canonical API origin. CLI and desktop use separate Sessions and credential entries. The config directory is not part of credential identity: two invocations targeting the same API share the CLI credential even if their config directories differ.

Persist changed `set-auth-token` response headers and carry the new token into subsequent requests. Invalid Sessions are cleared only if the stored credential still matches the failed request, preserving a newer login. Permission failures, server errors, and network failures do not delete valid credentials. Logout revokes the server Session before deleting its matching local entry; a failed remote logout remains retryable. OS credential writes and conditional deletion share a per-user file lock containing no secrets. Do not silently fall back to plaintext files. Linux requires a running, unlocked Secret Service; headless credential injection is deferred.

Project listing traverses all `/data/v1/projects` pages, including empty pages with a next cursor, and returns active readable sources. Detail uses `/api/v1/projects/{id}` and may describe an owned archived Project. Project names and descriptions provide context; they do not establish the actual Parquet schema.

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

The 2026-09-09 distribution decision selects Homebrew and WinGet only, with no npm package. Version 0.2.0 adds an embedded Skill installer and generates the package-manager metadata from actual archive hashes. The product repository stays private until launch; creation of a public tap, anonymous downloads and WinGet submission are deferred. No self-update daemon or extra CLI runtime dependency is introduced. See [package-manager distribution](package-managers.md).

## Confirmed public origins (2026-09-08; deployment pending)

The production web application will use `https://app.logcove.com`, and the public user API will use `https://api.logcove.com`. The website will use `https://logcove.com`; `https://docs.logcove.com` is reserved for a future documentation site. The shared Vector ingestion endpoint will be `https://ingest.logcove.com/logs`, authenticated with Project ID and write key, separate from the CLI's Session-authenticated API.

The test web application and API will use `https://app-test.logcove.com` and `https://api-test.logcove.com`; test ingestion will use `https://ingest-test.logcove.com/logs`. These are agreed target addresses, not a claim that services are deployed or reachable.

Update (2026-09-17): the source now includes `https://api.logcove.com` as the built-in fallback, after `--api-url`, `LOGCOVE_API_URL`, and the saved API origin. Upgrades preserve saved origins and origin-scoped credentials. Development and test environments use explicit overrides. Published v0.2.0 predates this runtime change and still requires an explicit API origin.

## Dependencies and maintenance

| Dependency | Reason and alternative |
| --- | --- |
| `clap` | Typed command parsing and help instead of handwritten argument parsing |
| `reqwest` with `rustls` | Blocking HTTPS and streaming downloads without system OpenSSL |
| `serde` / `serde_json` | Explicit API contracts and JSON serialization |
| `keyring` 3.6.3 | Same mature OS credential abstraction as the desktop, with an independent identity; plaintext persistence is not an automatic fallback |
| `directories` | OS-specific configuration paths without hand-coded platform rules |
| `open` | Open the existing browser with platform handling; manual links remain available |
| `time` | Parse calendar dates and timezone-aware link timestamps instead of maintaining handwritten leap-year and timestamp parsing; one pinned dependency with Rust-version and timezone-format checks |

Commit the application lockfile. Main maintenance costs are dependency/security updates, OS credential service differences, and cross-platform binary validation. Do not add async task orchestration, terminal UI frameworks, or a cross-repository shared SDK for this scope.

## Progress

- Batch 1 is implemented, with macOS tests, real browser authorization, native credential persistence, and local Project API verification completed. Linux and Windows verification boundaries are recorded in [validation.md](validation.md).
- Batch 2 download/manifest and Chart commands are implemented. macOS and Linux ARM64 automated tests pass. Real R2 downloads, DuckDB verification of 100,000 synthetic events, Chart result operations, and rendering in the existing web UI have passed; see [validation.md](validation.md).
- Batch 3 Skill authoring is implemented in `skills/logcove`, with binary-package installation instructions in [skills.md](skills.md) and actual verification in [skill-validation.md](skill-validation.md). Distribution batch 1 CI/build/package/release automation is implemented. All five platform jobs and Skill checks passed on GitHub Actions at `90ccde0` on 2026-09-09, with all six downloaded archives and checksum generation verified; see [hosted validation](releases.md#hosted-ci-and-artifact-validation-2026-09-09). The first binary release is v0.1.0, using explicit API configuration. Hosted test deployment and batch 2 installers/fresh-environment verification remain pending.
