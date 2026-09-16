# CLI usage

## Install

Download the prebuilt CLI for your platform from [v0.2.0](https://github.com/logcove/logcove-public/releases/tag/v0.2.0), extract it, and place `logcove` (`logcove.exe` on Windows) on PATH. Follow the [installation guide](../README.md#install) for macOS, Linux and Windows commands. Rust and a source checkout are not needed.

The CLI implements authentication, Project and write-key management, Parquet downloads, and Chart operations. Each CLI package also includes the [analysis Skill](skills.md). CLI commands do not require DuckDB; install DuckDB separately for local analysis. Source builds and development commands are in the [development guide](development.md#local-development).

Project mutations and `keys` commands below are implemented in the current source and are not in the published v0.2.0 binary. Check `logcove projects --help` and `logcove keys --help` before using them with an installed release.

## Install the bundled Skill (0.2.0+)

```sh
logcove skills install --agent codex
logcove skills install --agent claude
```

This local command works before configuring an API or logging in, including when an unrelated API configuration file is invalid. It installs the Skill embedded in this CLI into the selected agent's user directory. Repeating an identical installation is a no-op; different existing bundled files require explicit `--force`. It preserves extra files and rejects symlinked installation targets. See [Skill installation](skills.md#install-with-the-cli-020) for paths, output and overwrite behavior. The published 0.1.0 binary does not include this command.

## Select an API environment

Configure the actual origin supplied by your Logcove deployment:

```sh
logcove config set-api-url https://your-api.example.com
logcove config show
```

For local development:

```sh
logcove config set-api-url http://localhost:8787
```

No default production endpoint is assumed. Precedence is `--api-url`, `LOGCOVE_API_URL`, then the saved origin. A flag or environment override applies to that invocation and does not overwrite the saved configuration.

The agreed future production API is `https://api.logcove.com`, with the web application at `https://app.logcove.com`. After deployment and verification, a future release will use that API as a built-in fallback after the three overrides above. The test API is planned at `https://api-test.logcove.com`. These addresses are deployment plans, not currently verified service endpoints; the current CLI has no built-in default. Upgrades will preserve an explicitly saved local, test, or self-hosted origin.

```sh
logcove --api-url http://localhost:8787 whoami
```

Only HTTPS origins or loopback HTTP development origins are supported. Origins cannot include URL credentials, API paths, query strings, or fragments. Loopback API connections bypass proxies; remote API connections use reqwest's normal proxy support.

Configuration is `config.json` under the OS configuration directory plus `logcove`. `config show` reports its exact path. Override the directory with `--config-dir` or `LOGCOVE_CONFIG_DIR` for isolated development. The file contains only `api_url`; it never contains a Session.

## Login

```sh
logcove login
logcove whoami
```

Login opens your existing browser. Sign in with a supported account, review the code, and approve the CLI request. The authorization link and code are also printed to stderr. The CLI polls at the server's interval, observes rate-limit backoff, and stops on expiry or denial. The device code used for token exchange is not printed.

For SSH terminals or a browser you want to open manually:

```sh
logcove login --no-browser
```

Open the printed link on a device that can reach the deployment. A localhost link requires the corresponding local web application; it is not a remotely reachable test deployment. Ctrl-C stops the CLI; the pending browser request expires according to the server's expiry.

If already signed in, `login` returns the current account without creating another session. To switch accounts, log out of the CLI and begin a new authorization. The account used in the approval page is selected in the browser.

Successful login prints account JSON only:

```json
{"data":{"id":"user-example","email":"you@example.com","email_verified":true,"name":"Example","image":null,"created_at":"2026-09-08T00:00:00Z","updated_at":"2026-09-08T00:00:00Z"}}
```

## Credential persistence

The CLI uses a signed Bearer Session, not a JWT or a Vector write key. Credentials are stored in the OS credential service with service name `com.logcove.cli.session` and account equal to the canonical API origin.

| Platform | Backend | Requirement |
| --- | --- | --- |
| macOS | Keychain | Allow this CLI to access its own credential |
| Windows | Credential Manager | Run as the Windows user who logged in |
| Linux | Secret Service | A running and unlocked provider such as GNOME Keyring or KWallet, accessible through the user's session D-Bus |

API origins are separate identities, including ports. CLI and desktop credentials are independent; logging out of one does not log out of the other. Different config directories pointing at the same origin share that origin's CLI credential. Avoid switching to root/sudo to run the CLI: that changes the OS user and credential store.

The CLI saves changed `set-auth-token` headers returned by authenticated requests. A 401 clears the local credential only if it still matches the failed request's Session, preserving a newer login saved by another process. Permission failures, server errors, and network errors retain the credential. If saving a renewed credential fails, the operation still succeeds and stderr explains the persistence failure; the current process continues using the renewed token.

Credential writes and conditional deletion share a short cross-process file lock under the OS local-data directory at `logcove/credentials.lock`. The file contains no credential; the Session remains in the OS credential store. The lock also covers CLI invocations using different config directories.

If the initial login credential cannot be saved, login fails and attempts to revoke the newly issued session. It does not claim a persistent login succeeded. There is no automatic plaintext fallback. Headless noninteractive credentials are outside batch 1.

```sh
logcove logout
```

Logout revokes the CLI's server Session before deleting the matching local entry; a newer Session saved by another process is preserved. Remote failures leave it available for a retry. If the remote session is already invalid, remove only its matching stale local entry. A local deletion failure is reported distinctly; retry after unlocking the store. Calling logout when no credential exists succeeds without a network request.

## Discover Projects

```sh
logcove projects list
logcove projects get prj_00000000-0000-4000-8000-000000000001
```

The current CLI uses `/api/v1/projects`. `projects list` defaults to active Projects owned by the current account, including those without a write key. Use `--status archived` or `--status all` to include archived sources.

`list` follows every pagination cursor and emits one `{"data":[...]}` response. Empty pages do not terminate pagination when a next cursor exists. This is not a cross-page snapshot; concurrent Project changes may affect listing.

`get` returns `{"data":{...}}`. Both commands include `id`, `name`, `description`, `status`, `ingestion_protocol`, `data_prefix`, `write_key_id`, `ingestion.desired_revision`, `created_at`, and `updated_at` for each Project. Archived Project metadata is readable, but its log data is not. Names and descriptions provide analysis context; they do not substitute for reading the actual Parquet schema.

## Manage Projects and write keys

Each Project has one immutable `ingestion_protocol`: `http_json` (the default) or `otlp_http` (OTLP/HTTP Protobuf Logs). Choose it at creation:

```sh
logcove projects create --name "OTel logs" --ingestion-protocol otlp_http
```

Use `--ingestion-protocol http_json` for ordinary JSON. `projects update` does not accept the protocol; create a different Project to change formats. The same write Key may bind Projects of different protocols, but each request must use the matching Project and protocol endpoint. This flag requires the updated API; these source changes are not yet released. A created OTLP Project alone does not prove its collector endpoint is deployed. Use the endpoint supplied by that environment, never guess it from the API host.

All commands authenticate using the CLI's stored Session. Write keys authenticate Vector ingestion only; they cannot log in to the CLI or read data.

```sh
logcove projects create --name "API logs" --description "Backend request logs"
logcove keys create --name "API collector" \
  --project-id prj_00000000-0000-4000-8000-000000000001 \
  --output /path/to/private/api-collector.key
```

Use the returned Project ID, and choose a new output file in an existing directory outside source control. A Project starts active with no write key. Creating a Key can bind it to one or more Projects by repeating `--project-id`; omitting it creates an unbound Key. A Project can have one write key, while a Key can serve multiple Projects.

The Key creation command returns public metadata and an absolute `key_file` path:

```json
{"data":{"id":"key_00000000-0000-4000-8000-000000000001","name":"API collector","key_prefix":"lc_01234567","masked_key":"lc_01234567***","project_ids":["prj_00000000-0000-4000-8000-000000000001"],"revoked_at":null,"created_at":"2026-09-10T00:00:00Z","updated_at":"2026-09-10T00:00:00Z","key_file":"/path/to/private/api-collector.key"}}
```

The file contains the raw Key followed by a newline. The command never returns the plaintext Key or its hash on stdout/stderr. The destination is reserved before the API request and is never overwritten, including symlinks. Unix permissions are `0600`; Windows creates the file with a protected owner-only DACL. Windows alternate data streams are rejected. Keep this credential file private and pass its path to collector configuration code instead of copying its contents into an agent conversation. These files are separate from login Sessions, which remain in the OS credential store.

| Command | Behavior |
| --- | --- |
| `projects update <id> --name <name> --description <text>` | Update the supplied metadata fields; omitted fields stay unchanged |
| `projects update <id> --clear-description` | Clear the description |
| `projects update <id> --write-key-id <key-id>` | Bind or replace the Project's write key using its resource ID |
| `projects update <id> --clear-write-key` | Unbind the Project |
| `projects archive <id>` / `projects restore <id>` | Disable/restore data access without deleting stored logs |
| `keys list [--status active\|revoked\|all] [--project-id <id>]` | List masked metadata; defaults to all statuses and follows all pages |
| `keys get <id>` | Read metadata and bindings; cannot recover the secret |
| `keys update <id> --name <name>` | Rename without changing the secret |
| `keys set-projects <id> --project-id <id> ...` | Replace the entire binding set with the supplied Projects |
| `keys set-projects <id> --clear-projects` | Remove every binding without revoking the Key |
| `keys revoke <id>` | Irreversibly revoke and unbind; repeating is safe; `keys delete` is an alias |

Project metadata and write-key binding can be changed in one `projects update` request. `--clear-description` conflicts with `--description`; `--clear-write-key` conflicts with `--write-key-id`. `keys set-projects` requires either the complete list or explicit `--clear-projects`.

Creating a Key or replacing its Project set refuses a Project already bound to another Key. Use `projects update --write-key-id` for an intentional replacement. Bindings change atomically in the API. There is no hard-delete Project command. Revoking a Key does not delete its local credential file or log data.

Ingestion changes are asynchronous: `desired_revision` indicates the requested configuration, not proof that Vector has loaded it. The CLI does not poll for or claim immediate ingestion readiness.

### Key creation failures

- `KEY_FILE_ERROR`: the local destination could not be reserved; no creation request was sent.
- API rejection: the reserved file is removed on a best-effort basis. The CLI does not retry creation or print the response body.
- Lost/invalid response or server failure: creation may have occurred. Inspect `keys list` before retrying; a created secret cannot be retrieved again. Revoke an unusable Key by ID.
- `KEY_FILE_WRITE_FAILED`: the API created the Key, but local writing/syncing failed. The error identifies the Key and the `keys revoke <id>` recovery command. No automatic retry or revocation is performed; partial-file cleanup is best effort.

Success is returned only after the secret file has been written and synced. If stdout itself is interrupted afterward, the file may already contain the saved credential; inspect the chosen destination and Key metadata before retrying.

## Download Parquet

```sh
logcove data pull prj_00000000-0000-4000-8000-000000000001 \
  --from 2026-09-01 --to 2026-09-02 --output ./analysis/logs
```

Dates are inclusive UTC **ingestion partitions**, with a maximum range of 93 days. They do not filter event timestamps within a file. Inspect the schema and apply an event-time predicate in DuckDB if your question needs one. An archived or inaccessible Project cannot be downloaded.

The CLI follows all file-list pages and requests signed links in batches of at most 20 keys (also bounded by the API's request size). It streams up to four files concurrently by default, using shared connection pools for the pull. Use `--concurrency 1` for sequential downloads or choose any value from 1 to 8. File listing, signing, and Session handling remain on the main thread; only storage downloads run concurrently. The next signing batch starts after the current batch finishes. Loopback downloads bypass proxies; remote downloads use reqwest's normal proxy support. File bytes come directly from object storage without the user's Session attached. Download progress goes to stderr; stdout contains:

```json
{"data":{"project_id":"prj_00000000-0000-4000-8000-000000000001","manifest_path":"/your/output/pull-1788825600000000000-1234/manifest.json","file_count":1,"total_bytes":1024}}
```

Each invocation creates a new `pull-<time>-<process>` directory inside `--output`. It does not overwrite or resume an incomplete pull. Individual files use numeric local names. Filenames and manifest entries follow the file-list order, independent of download completion order. The manifest is published only after every file succeeds, including a valid empty manifest when there are no files. Read **only files listed in that manifest**, not a glob spanning old runs.

To refresh or extend a download while reusing local files, pass a completed manifest (repeat the flag for multiple runs of the same API and Project):

```sh
logcove data pull prj_00000000-0000-4000-8000-000000000001 \
  --from 2026-09-01 --to 2026-09-03 --output ./analysis/logs \
  --reuse-manifest ./analysis/logs/pull-previous/manifest.json
```

This option is implemented in source and is not yet released. Check `data pull --help` on your installed binary. Every pull still fetches the current authorized file listing. Reuse requires the same API origin, Project, object key, ETag and size, plus a local file of the expected size with Parquet markers. Missing, changed or invalid cached files are downloaded normally. Cached files no longer in the server listing are excluded. Mismatched or incomplete manifests return `INVALID_CACHE`. Reusing `--output` alone does not enable reuse.

Matching files are copied into the new run, without signing or downloading them, so the new manifest is complete and remains usable after the old run is removed. Copies consume local disk; this is not a global cache or automatic cleanup policy. Size/marker checks are not a cryptographic integrity check for local edits: treat completed downloads as immutable. Summary fields `downloaded_file_count` and `reused_file_count` distinguish network downloads from local copies; `file_count` and `total_bytes` describe the whole result. With all files cached, listing is still required but signing and storage downloads are skipped. Unchanged historical data can also be queried directly from an existing manifest when no refresh is needed.

```json
{
  "schema_version": 1,
  "api_url": "https://your-api.example.com",
  "project_id": "prj_00000000-0000-4000-8000-000000000001",
  "start_date": "2026-09-01",
  "end_date": "2026-09-02",
  "completed_at": "2026-09-03T00:00:00Z",
  "files": [
    {
      "key": "logs/project=prj_00000000-0000-4000-8000-000000000001/date=2026-09-01/example.parquet",
      "size": 1024,
      "etag": "example-etag",
      "uploaded_at": "2026-09-01T01:00:00Z",
      "ingest_date": "2026-09-01",
      "path": "000000.parquet"
    }
  ]
}
```

`path` is relative to the manifest's directory, so the directory can be moved as a unit. No signed URLs or credentials are saved. On Unix the run directory is private (0700) and files use 0600; Windows uses the user's normal filesystem ACLs.

The CLI checks ETags, byte counts, and Parquet header/footer markers. These checks detect incomplete or changed objects; DuckDB remains responsible for decoding their full contents. A changed ETag aborts the pull. Nearly expired links are refreshed, and a storage 403 gets one re-sign attempt. Storage failures do not remove the CLI login. API requests have a 30-second timeout; each download has a 10-second connection timeout and a 300-second total timeout.

Once the coordinator observes an unrecoverable download or signing failure, it stops starting new work and waits for all started downloads to finish or reach their existing timeout. A failed file's partial file is removed and no completed manifest is published. The error identifies the incomplete run directory; any files completed there, including other in-flight downloads that succeed during shutdown, remain for inspection or manual removal. Retry by running the same command, which starts a new run. Listing is not a storage snapshot: concurrent writes, retention, or replacements can affect a pull.

## Calculate locally

Use an independently installed DuckDB. For example, this Python setup creates a view using the full Project ID, after which the saved SQL can refer to that stable name:

```python
import json
from pathlib import Path
import duckdb

manifest_path = Path("analysis/logs/pull-example/manifest.json")
manifest = json.loads(manifest_path.read_text())
paths = [str(manifest_path.parent / f["path"]) for f in manifest["files"]]
if not paths:
    raise SystemExit("No files in the selected ingestion dates")
db = duckdb.connect()
db.read_parquet(paths, union_by_name=True).create_view(manifest["project_id"])
print(db.sql('DESCRIBE "' + manifest["project_id"] + '"').fetchall())
```

Inspect fields and samples before writing SQL. Register one view per downloaded Project for cross-source analysis. Chart `project_ids` declares all Project sources needed to initialize its tables/views. When recalculating, retrieve that list, authorize each read and register the sources in one DuckDB connection before executing SQL; the CLI/API do not initialize or execute it automatically. Dependencies do not grant access. Do not save local absolute Parquet paths or signed URLs into Chart SQL.

## Manage Charts

The working tree uses definition-only Charts; these changes are pending release and are newer than published v0.2.0. Results stay local. The removed `--result-file` option and `charts result put` command are no longer supported.

```sh
logcove charts list
logcove charts list --project-id prj_00000000-0000-4000-8000-000000000001
logcove charts get chart_00000000-0000-4000-8000-000000000001
logcove charts create --name "Requests by service" \
  --project-id prj_00000000-0000-4000-8000-000000000001 \
  --sql-file query.sql --spec-file chart.vl.json
logcove charts update chart_00000000-0000-4000-8000-000000000001 \
  --revision 1 --sql-file query.sql \
  --project-id prj_00000000-0000-4000-8000-000000000001
logcove charts delete chart_00000000-0000-4000-8000-000000000001
```

`list` follows all cursor pages and returns definition metadata. `get` also returns SQL and Vega-Lite spec; neither retrieves nor calculates results. Use the actual IDs and last observed revision. A stale revision fails without automatic retry. Omitted update fields are preserved; `--clear-description` explicitly clears a description.

Supply every source with repeated `--project-id`. Each SQL update requires the complete dependency list, or `--clear-projects` for a query with no Project sources. Creation without `--project-id` explicitly declares an empty list. The CLI/API do not infer sources from SQL. These dependencies do not grant data access.

The application filters every Project view by system `_created_time` using the selected half-open receipt-time range. Chart SQL queries those views without parameters, for example:

```sql
SELECT service, count(*) AS requests
FROM "prj_00000000-0000-4000-8000-000000000001"
GROUP BY service;
```

Use the actual time column and types. In Python, bind the selected UTC values with `db.execute(sql, {"start_time": start, "end_time": end})`. Register each source view in the same connection first. Saved SQL must not contain machine-specific paths, signed URLs or credentials. SQL/spec input files are UTF-8 (optional leading BOM), with limits of 64 KiB / 128 KiB; the total request limit is 256 KiB. The Vega-Lite spec uses only root `data: {"name":"result"}`, with no inline or external datasets. See the [Skill chart reference](../skills/logcove/references/charts.md) for a full spec and workflow.

The web/desktop Chart tab defaults to the last hour, with presets and a custom date/time picker. It downloads the selected UTC ingestion-date partitions, binds the time parameters and computes locally; this does not automatically include late arrivals from other dates. Successful results are cached in the application instance, and Refresh updates them. SQL/dependency changes use a new cache entry. The aggregate table and PNG/SVG exports use that local result. There is no cloud result upload, result history or background computation.

## Output and errors

Normal results are JSON on stdout. Login instructions and warnings use stderr. Runtime failures produce JSON on stderr and exit 1; argument/usage errors are handled by clap and exit 2. Help and version output are ordinary terminal text.

```json
{"error":{"code":"UNAUTHENTICATED","message":"No valid CLI session. Run logcove login for this API environment."}}
```

API errors include a request ID when the server supplies one. The CLI does not print raw HTTP error bodies, request headers, access tokens, or credential-store error details. HTTP redirects are refused; configure the final API origin instead.

Common codes include `API_NOT_CONFIGURED`, `INVALID_API_URL`, `UNAUTHENTICATED`, `ACCESS_DENIED`, `NOT_FOUND`, `RATE_LIMITED`, `NETWORK_ERROR`, `SERVER_ERROR`, `AUTHORIZATION_DENIED`, `AUTHORIZATION_EXPIRED`, and credential-store errors. Download/input failures additionally include `INVALID_DATE_RANGE`, `FILE_ERROR`, `DOWNLOAD_FAILED`, `DOWNLOAD_DENIED`, `OBJECT_CHANGED`, `INVALID_PARQUET`, `INVALID_INPUT`, and `PAYLOAD_TOO_LARGE`. Only authentication failure is a reason to log in again; a 403 or 5xx does not mean the Session expired.

## Development checks

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --release --locked
```

Normal tests use fake credentials and a local HTTP server, not real accounts. An explicit OS-store smoke test uses disposable fake credentials under a unique `.invalid` origin and separate processes, then removes its entries:

```sh
cargo test --locked --test native_credentials -- --ignored --nocapture
```

Run that test only in a configured OS credential environment. CI performs build and mocked HTTP/credential checks on macOS, Linux, and Windows; it does not assume hosted runners have an unlocked desktop keyring. Actual verification results are recorded in [validation.md](validation.md).
