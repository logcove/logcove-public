# Implementation plan

Agreed on 2026-09-08. This document distinguishes planned capabilities from implemented and verified behavior; a plan is not a release announcement.

## Goal and repository boundary

Complete the following workflow with a local coding agent:

```text
Browser authorization -> Project discovery -> Parquet download
  -> local DuckDB analysis -> Vega-Lite specification
  -> save Chart and result -> view in the web application
```

This repository holds a single Rust CLI crate under `cli/`, an analysis Skill under `skills/logcove/`, public user documentation under `docs/`, and CI/release configuration under `.github/workflows/`. The license is Apache-2.0.

The CLI depends only on public HTTP APIs. It does not read service databases, call ingestion control endpoints, or depend on private application source code. Deployment details, credentials, actual user logs, and infrastructure experiments do not belong in this repository.

## Responsibilities

The CLI implements deterministic operations: authentication, pagination, file downloading, Chart definitions, and result uploads. The Skill guides the agent in selecting data, inspecting schemas, executing DuckDB, writing SQL, producing Vega-Lite, and checking results.

There is no CLI query engine, `data describe` command, bundled DuckDB, LLM client, background daemon, or SDK/code generation layer. The agent uses an independently installed DuckDB environment. A future container can install the released CLI and the same Skill; noninteractive service identity is a later design.

## Command scope

| Command | Batch | Responsibility |
| --- | --- | --- |
| `config show` / `config set-api-url <origin>` | 1 | Configure the target API without storing credentials in configuration |
| `login [--no-browser]` | 1 | Authorize in the user's browser and save a signed Session |
| `whoami` / `logout` | 1 | Inspect the current identity / revoke this CLI session |
| `projects list` / `projects get <id>` | 1 | Discover readable active Projects / inspect Project metadata |
| `data pull <project-id> --from <date> --to <date> --output <dir> [--concurrency <1-8>]` | 2 | Download Parquet for inclusive UTC ingestion dates |
| `charts list/get/create/update/delete` | 2 | Manage Chart definitions |
| `charts result put <id> --file <result.json>` | 2 | Replace the latest result |

Normal command output is stable JSON on stdout. Login instructions, progress, and warnings go to stderr. Failures return a nonzero exit code. Tokens and signed download URLs are not normal output. SQL, Vega-Lite specifications, and results are accepted through files in batch 2 to avoid shell escaping.

## Batch 1: authentication and discovery

Configuration precedence is `--api-url`, then `LOGCOVE_API_URL`, then the saved API origin. No undeployed production endpoint is assumed. `--config-dir` / `LOGCOVE_CONFIG_DIR` allow an isolated configuration directory; otherwise use the OS configuration directory plus `logcove`. Configuration contains only the API origin.

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

Chart creation may include its initial computed result. Definition updates respect the existing revision conflict contract. SQL text changes clear the previous result; style changes keep it. Results have no history/version record, thumbnail, query parameters, or scheduled refresh. A Vega-Lite spec uses `data: {"name":"result"}`. Result uploads contain `computed_at` and an array of at most 10,000 objects / 5 MiB; upload aggregated results, not all raw logs.

Batch 2 acceptance: real authorized Parquet downloads, known DuckDB aggregates, Chart creation/update/result replacement, and successful web rendering. The test API's R2 listing binding and download-signing target must point to the same real storage; locally emulated R2 cannot validate real downloads.

## Batch 3: Skill and distribution

Create one `skills/logcove/SKILL.md` with the core workflow. Put detailed CLI usage, DuckDB examples, and Vega-Lite conventions in `references/` for on-demand reading. Maintain a common core for Codex and Claude Code with separate installation instructions.

The Skill checks identity, discovers sources, scopes the question and time range, downloads files, inspects schemas/samples, computes and checks results, and creates a Chart when the user wants one saved. Service interactions use the CLI; prompts do not ask agents to assemble authentication headers or handle Session secrets.

Distribute prebuilt binaries via GitHub Releases and include checksums, license, and matching CLI/Skill documentation. Start development on macOS while checking macOS, Linux, and Windows builds. Record actual supported release targets and tested runtimes. A stable HTTPS test deployment is required before an external test release; local development is not blocked by that deployment.

## Dependencies and maintenance

| Dependency | Reason and alternative |
| --- | --- |
| `clap` | Typed command parsing and help instead of handwritten argument parsing |
| `reqwest` with `rustls` | Blocking HTTPS and streaming downloads without system OpenSSL |
| `serde` / `serde_json` | Explicit API contracts and JSON serialization |
| `keyring` 3.6.3 | Same mature OS credential abstraction as the desktop, with an independent identity; plaintext persistence is not an automatic fallback |
| `directories` | OS-specific configuration paths without hand-coded platform rules |
| `open` | Open the existing browser with platform handling; manual links remain available |
| `time` | Parse calendar dates and timezone-aware result/link timestamps instead of maintaining handwritten leap-year and timestamp parsing; one pinned dependency with Rust-version and timezone-format checks |

Commit the application lockfile. Main maintenance costs are dependency/security updates, OS credential service differences, and cross-platform binary validation. Do not add async task orchestration, terminal UI frameworks, or a cross-repository shared SDK for this scope.

## Progress

- Batch 1 is implemented, with macOS tests, real browser authorization, native credential persistence, and local Project API verification completed. Linux and Windows verification boundaries are recorded in [validation.md](validation.md).
- Batch 2 download/manifest and Chart commands are implemented. macOS and Linux ARM64 automated tests pass. Real R2 downloads, DuckDB verification of 100,000 synthetic events, Chart result operations, and rendering in the existing web UI have passed; see [validation.md](validation.md).
- Batch 3 Skill/distribution and hosted test deployment remain pending. No CLI binary release has been published.
