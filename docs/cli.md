# CLI usage

## Install

Download the prebuilt CLI for your platform from [v0.3.2](https://github.com/logcove/logcove-public/releases/tag/v0.3.2), extract it, and place `logcove` (`logcove.exe` on Windows) on PATH. Follow the [installation guide](../README.md#install) for macOS, Linux and Windows commands. Rust and a source checkout are not needed.

The CLI implements authentication, Project and write-key management, Parquet downloads, and Chart operations. Each CLI package also includes the [analysis Skill](skills.md). CLI commands do not require DuckDB; install DuckDB separately for local analysis. Source builds and development commands are in the [development guide](development.md#local-development).

Project mutations and `keys` commands below require CLI 0.3.0 or newer. Check `logcove projects --help` and `logcove keys --help` before using them with an installed release.

## Install the bundled Skill (0.2.0+)

```sh
logcove skills install --agent codex
logcove skills install --agent claude
```

This local command works before configuring an API or logging in, including when an unrelated API configuration file is invalid. It installs the Skill embedded in this CLI into the selected agent's user directory. Repeating an identical installation is a no-op; different existing bundled files require explicit `--force`. It preserves extra files and rejects symlinked installation targets. See [Skill installation](skills.md#install-with-the-cli-020) for paths, output and overwrite behavior. The published 0.1.0 binary does not include this command.

## Select an API environment

The CLI defaults to `https://api.logcove.com`. Ordinary users can run `logcove login` directly. Inspect the effective address with:

```sh
logcove config show
```

For local development:

```sh
logcove config set-api-url http://localhost:8787
```

Precedence is `--api-url`, `LOGCOVE_API_URL`, the saved origin, then the built-in production address. A flag or environment override applies to that invocation and does not overwrite the saved configuration. Invalid explicit configuration reports an error rather than silently switching to production.

The production web application is `https://app.logcove.com`. The default API is included in CLI 0.3.0; older releases require explicit configuration. Upgrades preserve explicitly saved local, test, or self-hosted origins and their separate credentials.

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

The current source implementation stores a signed Bearer Session in
`~/.logcove/auth.json` (`%USERPROFILE%\.logcove\auth.json` on Windows). This is
not an account password, JWT or Vector write key. **This change is not released
in v0.3.2**: published v0.3.2 and earlier binaries use the OS credential store.
See the [file-credential plan and migration](file-credentials.md).

After upgrading to a file-based build, run `logcove login` once. The CLI does not
read, migrate or delete old Keychain/Credential Manager/Secret Service entries.
Desktop login is unchanged. File-based builds need no desktop keyring service.

The file contains a `sessions` map keyed by canonical API origin, including
ports. CLI and desktop Sessions are independent. `--config-dir` and
`LOGCOVE_CONFIG_DIR` still select non-secret configuration only; different config
directories targeting the same API share the same user's CLI Session.

Unix credential directories are `0700` and files are `0600`; Windows credential
files are created with a protected owner-only DACL. The tokens are plaintext:
processes running as your user can read them. Never copy `auth.json` into source
control, tool output or agent conversations.

All store operations take `~/.logcove/credentials.lock`. Updates write and sync a
private temporary file, then replace `auth.json` in the same directory. The lock
contains no secrets. Missing files mean logged out; malformed files and storage
errors are reported without exposing their contents or silently resetting them.

Changed `set-auth-token` headers are persisted. A 401 clears only the matching
stored Session, preserving a newer login from another process. Permission,
server and network failures retain the Session. Renewal persistence failures
produce a warning while the successful request and in-memory renewal remain
usable. Initial login persistence failure attempts to revoke the new server
Session and does not claim persistent login succeeded.

`LOGCOVE_TOKEN` takes priority and never reads or writes the Session file.

```sh
logcove logout
```

Logout revokes the CLI's server Session before deleting the matching local entry; a newer Session saved by another process is preserved. Remote failures leave it available for a retry. If the remote session is already invalid, remove only its matching stale local entry. A local deletion failure is reported distinctly; retry after correcting the reported credential-storage problem. Calling logout when no credential exists succeeds without a network request.

## Discover Projects

```sh
logcove projects list
logcove projects get prj_00000000-0000-4000-8000-000000000001
```

The current CLI uses `/api/v1/projects`. `projects list` defaults to active Projects owned by the current account, including those without a write key. Use `--status archived` or `--status all` to include archived sources.

`list` follows every pagination cursor and emits one `{"data":[...]}` response. Empty pages do not terminate pagination when a next cursor exists. This is not a cross-page snapshot; concurrent Project changes may affect listing.

`get` returns `{"data":{...}}`. Both commands include `id`, `name`, `description`, `status`, `ingestion_protocol`, `data_prefix`, `write_key_id`, `ingestion.desired_revision`, `created_at`, and `updated_at` for each Project. Archived Project metadata is readable, but its log data is not. Names and descriptions provide analysis context; they do not substitute for reading the actual Parquet schema.

Starting with CLI 0.3.1, Project responses preserve the read-only `usage` metadata
returned by the service. `raw_bytes` is the normalized, uncompressed UTF-8 JSON
written over the latest 90 UTC calendar dates, excluding platform root fields.
`start_date` and `end_date` define that window; `tracking_started_at` marks when
accounting began for the Project. `updated_at` is the last reported increase in
the window, or null if none. Reports update asynchronously and do not backfill
older logs. This total is neither current R2 storage size nor the account's
billing-period usage; file listings and manifests still report Parquet bytes.

## Manage Projects and write keys

Each Project has one immutable `ingestion_protocol`: `http_json` (the default) or `otlp_http` (OTLP/HTTP Protobuf Logs). Choose it at creation:

```sh
logcove projects create --name "OTel logs" --ingestion-protocol otlp_http
```

Use `--ingestion-protocol http_json` for ordinary JSON. `projects update` does not accept the protocol; create a different Project to change formats. The same write Key may bind Projects of different protocols, but each request must use the matching Project and protocol endpoint. This flag requires CLI 0.3.0 and an API with OTLP Project support. A created OTLP Project alone does not prove its collector endpoint is deployed. Use the endpoint supplied by that environment, never guess it from the API host.

Business commands authenticate using `LOGCOVE_TOKEN` when set, or the CLI's stored Session otherwise. The current v0.3.2 and production API support PAT authentication. Write keys authenticate Vector ingestion only; they cannot log in to the CLI or read data.

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

The file contains the raw Key followed by a newline. The command never returns the plaintext Key or its hash on stdout/stderr. The destination is reserved before the API request and is never overwritten, including symlinks. Unix permissions are `0600`; Windows creates the file with a protected owner-only DACL. Windows alternate data streams are rejected. Keep this credential file private and pass its path to collector configuration code instead of copying its contents into an agent conversation. These files are separate from the CLI login Session file.

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

The CLI follows all file-list pages and requests signed links in batches of at most 20 keys (also bounded by the API's request size). It streams up to four files concurrently by default, using shared connection pools for the pull. Use `--concurrency 1` for sequential downloads or choose any value from 1 to 8. File listing, signing, and Session handling remain on the main thread; only storage downloads run concurrently. The next signing batch starts after the current batch finishes. Loopback downloads bypass proxies; remote downloads use reqwest's normal proxy support. File bytes come directly from object storage without the user's Session or personal token attached. Download progress goes to stderr; stdout contains:

```json
{"data":{"project_id":"prj_00000000-0000-4000-8000-000000000001","manifest_path":"/your/output/pull-1788825600000000000-1234/manifest.json","file_count":1,"total_bytes":1024}}
```

Each invocation creates a new `pull-<time>-<process>-<attempt>` directory inside `--output`. It does not overwrite an earlier invocation or resume its incomplete pull. Individual files use numeric local names. Filenames and manifest entries follow the file-list order, independent of download completion order. The manifest is published only after every file succeeds, including a valid empty manifest when there are no files. Read **only files listed in that manifest**, not a glob spanning old runs.

To refresh or extend a download while reusing local files, pass a completed manifest (repeat the flag for multiple runs of the same API and Project):

```sh
logcove data pull prj_00000000-0000-4000-8000-000000000001 \
  --from 2026-09-01 --to 2026-09-03 --output ./analysis/logs \
  --reuse-manifest ./analysis/logs/pull-previous/manifest.json
```

This option requires CLI 0.3.0 or newer. Check `data pull --help` on your installed binary. Every pull still fetches the current authorized file listing. Reuse requires the same API origin, Project, object key, ETag and size, plus a local file of the expected size with Parquet markers. Missing, changed or invalid cached files are downloaded normally. Cached files no longer in the server listing are excluded. Mismatched or incomplete manifests return `INVALID_CACHE`. Reusing `--output` alone does not enable reuse.

Matching files are copied into the new run, without signing or downloading them, so the new manifest is complete and remains usable after the old run is removed. Copies consume local disk; this is not a global cache or automatic cleanup policy. Size/marker checks are not a cryptographic integrity check for local edits: treat completed downloads as immutable. Summary fields `downloaded_file_count` and `reused_file_count` distinguish network downloads from local copies; `file_count` and `total_bytes` describe the whole result. With all files cached, listing is still required but signing and storage downloads are skipped. Unchanged historical data can also be queried directly from an existing manifest when no refresh is needed.

```json
{
  "schema_version": 1,
  "api_url": "https://api.logcove.com",
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

Once the coordinator observes an unrecoverable download or signing failure, it stops starting new work and waits for all started downloads to finish or reach their existing timeout. A failed file's partial file is removed and no completed manifest is published. The error identifies the incomplete run directory; any files completed there, including other in-flight downloads that succeed during shutdown, remain for inspection or manual removal. Retry by running the same command, which starts a new run. CLI 0.3.2 and newer recognize `409 FILE_SET_CHANGED`, file-level `404 FILE_UNAVAILABLE`, and storage GET 404. It discards all previous pages and restarts the entire pull once, including fresh authorization. It can reuse completed validated downloads from the first attempt only when the new listing has the same key, ETag and size. Each attempt has a separate directory; only the successful attempt gets a completed manifest. A second such failure is returned, without further automatic retries. Project 404, permission failures, SQL errors and generic conflicts are not file-refresh signals.

A refresh may replace many small-file keys with different keys covering the same logs. Query only the final manifest, never combine it with the old file set. Compaction-aware pagination detects publication changes, but ordinary new uploads still do not provide a transactional storage snapshot.

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

CLI 0.3.0 uses definition-only Charts, replacing the result-upload contract in v0.1.x and v0.2.x. Results stay local. The removed `--result-file` option and `charts result put` command are no longer supported.

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

For local Chart verification, first register each source view in the same connection with the selected UTC bounds applied to `_created_time`, following the [filtered-view example](../skills/logcove/references/duckdb.md#register-downloaded-sources). Then execute the saved SQL with `db.execute(sql)`, without a parameter dictionary. Save a single SELECT query (including WITH/CTEs); keep setup statements outside it. Saved SQL must not contain machine-specific paths, signed URLs or credentials. SQL/spec input files are UTF-8 (optional leading BOM), with limits of 64 KiB / 128 KiB; the total request limit is 256 KiB. The Vega-Lite spec uses only root `data: {"name":"result"}`, with no inline or external datasets. See the [Skill chart reference](../skills/logcove/references/charts.md) for a full spec and workflow.

The web/desktop Chart tab defaults to the last hour, with presets and a custom date/time picker. It downloads the selected UTC ingestion-date partitions, filters Project views by `_created_time` and computes locally; this does not automatically include late arrivals from other dates. Successful results are cached in the application instance, and Refresh updates them. SQL/dependency changes use a new cache entry. The aggregate table and PNG/SVG exports use that local result. There is no cloud result upload, result history or background computation.

## Output and errors

Known API conflicts retain their business codes: `PROJECT_LIMIT_REACHED` requires freeing an active Project slot or upgrading, `CHART_CONFLICT` requires reconciling the latest revision, and `WRITE_KEY_CONFLICT`, `INVALID_KEY_BINDING`, or `KEY_REVOKED` require inspecting current bindings or selecting an active Key. The CLI uses fixed messages for these codes and does not print arbitrary server response bodies. Unknown conflicts remain `CONFLICT`.

Normal results are JSON on stdout. Login instructions and warnings use stderr. Runtime failures produce JSON on stderr and exit 1; argument/usage errors are handled by clap and exit 2. Help and version output are ordinary terminal text.

```json
{"error":{"code":"UNAUTHENTICATED","message":"No valid CLI session. Run logcove login for this API environment."}}
```

API errors include a request ID when the server supplies one. The CLI does not print raw HTTP error bodies, request headers, access tokens, or credential-store error details. HTTP redirects are refused; configure the final API origin instead.

Common codes include `INVALID_API_URL`, `UNAUTHENTICATED`, `ACCESS_DENIED`, `NOT_FOUND`, `RATE_LIMITED`, `NETWORK_ERROR`, `SERVER_ERROR`, `AUTHORIZATION_DENIED`, `AUTHORIZATION_EXPIRED`, and credential-store errors. Download/input failures additionally include `INVALID_DATE_RANGE`, `FILE_ERROR`, `DOWNLOAD_FAILED`, `DOWNLOAD_DENIED`, `OBJECT_CHANGED`, `INVALID_PARQUET`, `FILE_SET_CHANGED`, `FILE_UNAVAILABLE`, `INVALID_INPUT`, and `PAYLOAD_TOO_LARGE`. For Session mode, only authentication failure is a reason to log in again; a 403 or 5xx does not mean the Session expired. In token mode, correct `LOGCOVE_TOKEN` instead of starting browser login.

## Development checks

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --release --locked
```

Normal tests use fake credentials and local HTTP fixtures, not real accounts.
File-store checks use isolated directories and include process-to-process
persistence, concurrent writers, permissions and conditional deletion:

```sh
cargo test --locked --test file_credentials
```

These tests run in normal CI without a desktop keyring. Actual platform runtime
verification is recorded in [validation.md](validation.md).


## Personal tokens for automation

CLI v0.3.1 supports `LOGCOVE_TOKEN`, and the production API/app has personal-token
support deployed. The September 19 replacement v0.3.0 also supports PATs, while
the original v0.3.0 binaries do not. For an older installation, check root help
for `LOGCOVE_TOKEN` or upgrade to v0.3.1; the v0.3.0 version number alone does not
establish support. Browser Session login remains available.

Create a named token under **Personal tokens** in the app. Its secret appears once;
store it in your CI or remote agent secret store and inject it as `LOGCOVE_TOKEN`.
Run `logcove whoami` to verify the account, then use normal Projects, Keys, Charts
and Data commands. No browser login or Linux Secret Service is needed in this mode.

```yaml
# After installing a CLI version that supports personal tokens:
- name: List Logcove projects
  env:
    LOGCOVE_TOKEN: ${{ secrets.LOGCOVE_TOKEN }}
  run: logcove projects list
```

All tokens grant Full access to the owner's business resources, without bypassing
quotas or retention. They cannot manage personal tokens, account security or billing.
They have no automatic expiry; revoke individual tokens in the app. Collector
write keys remain separate and cannot be used as `LOGCOVE_TOKEN`.

Credential precedence: `LOGCOVE_TOKEN` before the saved Session. The environment
credential is never persisted, renewed or printed. Invalid/empty values fail with
`INVALID_TOKEN`; rejected values return `UNAUTHENTICATED` without deleting or using
a saved Session. Token mode ignores Session-renewal headers. `login`/`logout` fail
with `ENV_TOKEN_ACTIVE`; unset the variable to operate on the saved browser Session.
Unsetting it or logging out of a Session does not revoke the PAT.
