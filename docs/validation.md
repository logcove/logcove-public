# CLI validation

Date: 2026-09-08. This is local development verification, not a hosted deployment or binary release.

## Implemented

- API environment configuration and per-invocation overrides.
- Browser device authorization with manual-link support, polling/backoff, expiry, and denial handling.
- Independent CLI Sessions in the OS credential store, renewal, and logout.
- Readable Project discovery with automatic pagination and Project metadata lookup.
- Parquet downloads with bounded signing batches, temporary files, and a completed manifest.
- Chart definition CRUD, explicit revision updates, and latest-result upload.
- Stable JSON results, structured runtime errors, and redacted diagnostics.

## Verified locally

| Check | Result |
| --- | --- |
| macOS ARM64 / Rust 1.90 compilation | Passed |
| macOS release build | Passed |
| rustfmt and clippy with warnings denied | Passed |
| Configuration and executable-level tests | 6 passed |
| HTTP/authentication/Project contract tests | 21 passed |
| Download/date/manifest contract tests | 11 passed |
| Chart/input/result contract tests | 10 passed |
| macOS real Keychain persistence | Passed using disposable fake credentials in separate processes, including conditional deletion preserving a newer credential; entries removed afterward |
| Real browser authorization against a local Logcove API | Passed using `login --no-browser` and the existing browser approval page |
| Real Session restore in subsequent CLI processes | Passed |
| Real Project list and detail | Passed; four active readable sources, including the synthetic-log source |
| Real missing Project and subsequent Session reuse | Passed; structured NOT_FOUND did not clear the Session |
| Re-running login while already authenticated | Passed without another browser authorization |
| Real local API Chart definition CRUD | Passed: create/get/tag-filtered list/update/clear fields/stale revision/delete/404; disposable Chart removed |
| Real local API with emulated R2 result storage | Passed: create with result/get/style retains result/SQL clears result/result replacement |
| Existing web UI rendering a CLI-created Chart | Passed with the local two-row fixture: two Vega bars (12 and 7), zero page errors; temporary Chart deleted afterward |
| Real R2 Parquet download through the CLI | Passed: 56 files, 3,284,233 bytes, a completed manifest |
| DuckDB verification of the downloaded files | Passed: 100,000 rows, 100,000 unique events, sequence 0-99,999, no wrong Project/run rows |
| Chart results persisted in real R2 | Passed: create/get/style retains result/SQL clears result/result replacement |
| Existing web UI rendering the real aggregate | Passed: four service bars matching the DuckDB counts, zero page errors; Chart retained |

The real-user CLI Session is retained for continued development. Logout/revocation is covered by HTTP contract tests, not by signing out that retained live session. Browser auto-open is implemented through `open`; the real browser test used a manually opened link.

Normal tests use a local HTTP stub and fake credential storage. Covered failure paths include pending/slow-down/rate-limit authorization, denial, expiry, consumed grants, unexpected unsigned tokens, unsafe verification URLs, initial credential-save failure and revocation, Session renewal persistence errors, logout remote/local failures, empty cursor pages, repeated cursors, mid-pagination expiry, forbidden/not-found/server errors, network failures, and redirect refusal.

Download checks cover inclusive UTC calendar dates and the 93-day boundary, multi-page listing including empty intermediate pages, 20-key and 16 KiB signing batch limits, auth-free storage requests, three downloads sharing one keep-alive connection, manifest-relative paths, unrelated old files, empty datasets, refresh before using an expired link, a single re-sign after a storage 403, failed-file cleanup without publishing a manifest, changed ETags, redirect refusal, foreign Project keys, and repeated cursors.

Chart checks cover create with initial data, omitted versus explicitly cleared update fields, SQL-change null results, explicit upload format, a stale revision without automatic retry, empty DELETE responses, filtered pagination, result responses larger than 2 MiB, input sizes, and invalid IDs. Additional input cases verify UTC/millisecond timestamp normalization, invalid and future dates, leading UTF-8 BOM removal for SQL/spec/result files, preserved interior BOM characters, unchanged source files, and size limits after BOM removal. These contract tests do not substitute for a real R2 and browser test.

Credential regression tests verify that an old process receiving a 401 or logging out preserves a newer login in the shared store. The native macOS smoke test also verifies conditional deletion against Keychain. The per-user lock uses Rust's standard file locking without adding a dependency.

After these review fixes, macOS and Linux ARM64 each passed all 48 automated tests; macOS clippy, formatting, release build, and native Keychain smoke also passed. Two real-storage download attempts completed 20 and 40 files respectively, then received HTTP 500 while the API checked R2 object metadata for the next signing batch (`head_files`). The first failure was correlated with an R2 500 in the development Worker log. Both incomplete runs retained completed files without publishing a manifest or leaving partial files. The full 56-file success recorded above predates these fixes; this rerun does not establish a complete real-storage pass for the updated build.

## Batch 2 integration status

The development API uses a real remote R2 binding, with its download-signing target pointing at that same bucket. D1 and API execution remain local. The two existing local Chart result fixtures were preserved in real R2 before switching, then read successfully through the remote binding. `wrangler dev --local` disables remote bindings and must not be used for this setup.

Native remote-binding requests initially timed out while `workers.dev` was unreachable directly. The user enabled TUN, direct access recovered, and the complete real-storage workflow then passed. The active development API now uses that working remote configuration. This was a local networking issue; it did not require changes to the user API or an R2 transport workaround.

The retained synthetic source was downloaded with `data pull`. DuckDB read only the manifest's files, verified all 100,000 events, and aggregated request counts, errors, error rates, and average durations by service. The saved SQL refers to a view named by the full Project ID, with no local file paths. The CLI saved the spec and four result rows, read them back, verified style changes preserve results and SQL changes clear them, then uploaded and read the result again.

Browser checks used the installed Playwright library and fresh headless Chrome contexts because the connected browser tools were unavailable. They signed into the existing test account and read CLI-created Charts without mocking API responses. Both the small local fixture and the real R2 aggregate rendered correctly. The local fixture was deleted; the real aggregate Chart and downloaded Parquet were retained for continued analysis.

## Platform boundaries

- Linux ARM64 compilation and all 48 mocked/command tests passed inside the official Rust 1.90 Bookworm container, including download and Chart contracts.
- Linux native Secret Service persistence has not been run for this CLI. The container tests intentionally skip the explicit OS-store smoke test; users need a running and unlocked provider.
- Local Windows GNU cross-check was attempted but could not complete because `x86_64-w64-mingw32-gcc` is unavailable for the TLS dependency build. This is not a passed Windows build or runtime test.
- CI is configured for native macOS, Linux, and Windows checks, tests, and release compilation. No CI run is claimed before the repository is pushed.

The actual Windows credential store, Windows browser open behavior, and distribution/install experience remain platform/release acceptance work.

## Reproduce

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo test --locked --test native_credentials -- --ignored --nocapture
cargo build --release --locked
```

Run the explicit native credential test only in a configured OS credential environment. It uses a unique `.invalid` API identity, not an existing user's login entry.

Local integration reports and non-secret development configuration stay in the ignored `experiments/` directory. They are not part of the public source distribution. No user tokens, logs, or private infrastructure settings are included here.
