# CLI usage

## Build

Use Rust 1.90 or newer:

```sh
cargo build --locked
./target/debug/logcove --help
```

To install a local build in Cargo's executable directory:

```sh
cargo install --path cli --locked
```

Prebuilt releases are not published yet. The CLI implements authentication, Project discovery, Parquet downloads, and Chart operations; the [analysis Skill](skills.md) is available from this checkout. These commands do not require DuckDB; install DuckDB separately for local analysis.

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

`list` follows every cursor from the readable Project API and emits one `{"data":[...]}` response. It returns active Projects owned by the current account, even if they have no write key. Empty pages do not terminate pagination when a next cursor exists. This is not a cross-page snapshot; concurrent Project changes may affect listing.

`get` returns `{"data":{...}}` containing `id`, `name`, `description`, `status`, `data_prefix`, `created_at`, and `updated_at`. It may describe an owned archived Project; that does not make archived data readable. It does not expose Key bindings or ingestion control metadata. Names and descriptions provide analysis context; they do not substitute for reading the actual Parquet schema.

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

Each invocation creates a new `pull-<time>-<process>` directory inside `--output`. It does not reuse, resume, or overwrite a previous pull. Individual files use numeric local names. Filenames and manifest entries follow the file-list order, independent of download completion order. The manifest is published only after every file succeeds, including a valid empty manifest when there are no files. Read **only files listed in that manifest**, not a glob spanning old runs.

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

Inspect fields and samples before writing SQL. Register one view per downloaded Project for cross-source analysis. This view naming is a client convention; the server does not automatically register or execute saved SQL. Chart `project_ids` are metadata tags and do not grant access. Do not save local absolute Parquet paths or signed URLs into Chart SQL.

## Manage Charts

```sh
logcove charts list
logcove charts list --project-id prj_00000000-0000-4000-8000-000000000001
logcove charts get chart_00000000-0000-4000-8000-000000000001
```

`list` follows all cursor pages and returns summaries, optionally filtered by a Project tag. `get` returns the definition, revision, result metadata, and current result data (or `null` when no result exists). It does not run SQL. The web application can render that stored result immediately.

Prepare UTF-8 input files. SQL, spec, and result files may start with a UTF-8 BOM; the CLI strips that leading marker before parsing and enforcing content size limits. It preserves the rest of the text and does not modify the source file. UTF-16 input is unsupported. For data with a `service` field, `query.sql` could contain:

```sql
SELECT service, count(*) AS requests
FROM "prj_00000000-0000-4000-8000-000000000001"
GROUP BY service
ORDER BY requests DESC;
```

`chart.vl.json` uses the named `result` dataset:

```json
{
  "$schema": "https://vega.github.io/schema/vega-lite/v6.json",
  "data": {"name": "result"},
  "mark": "bar",
  "encoding": {
    "x": {"field": "service", "type": "nominal"},
    "y": {"field": "requests", "type": "quantitative"}
  }
}
```

The API also validates nested data-source positions. Inline datasets and external data URLs are unsupported. A result file is an explicit object containing exactly `computed_at` and `data`, not a bare row array:

```json
{"computed_at":"2026-09-03T00:00:00Z","data":[{"service":"api","requests":12}]}
```

Generate the timestamp at computation time in RFC 3339 format with a timezone. The CLI converts it to UTC and truncates sub-millisecond digits before upload, accepting Python's usual microsecond output without changing the source file. Invalid dates, missing timezones, and timestamps more than five minutes in the future are rejected. Upload JSON-compatible aggregate rows, with at most 10,000 object rows and 5 MiB of serialized data. SQL is limited to 64 KiB and the spec input file to 128 KiB. Avoid NaN/Infinity in results.

For example, after registering the view in the Python setup:

```python
from datetime import datetime, timezone

cursor = db.execute(Path("query.sql").read_text())
columns = [column[0] for column in cursor.description]
rows = [dict(zip(columns, row)) for row in cursor.fetchall()]
result = {
    "computed_at": datetime.now(timezone.utc).isoformat(timespec="milliseconds"),
    "data": rows,
}
Path("result.json").write_text(
    json.dumps(result, ensure_ascii=False, allow_nan=False, separators=(",", ":")),
    encoding="utf-8",
)
```

Cast timestamp, decimal, or other non-JSON result columns explicitly in SQL or convert them deliberately before serializing; the example query returns only strings and integer counts.

```sh
logcove charts create --name "Requests by service" \
  --description "Requests in the selected ingestion dates" \
  --project-id prj_00000000-0000-4000-8000-000000000001 \
  --sql-file query.sql --spec-file chart.vl.json --result-file result.json
```

`--description`, `--project-id`, and `--result-file` are optional; repeat `--project-id` for multiple tags. Creating without a result stores the definition with an empty result. Creating with a result submits them together through the existing API.

```sh
logcove charts update chart_00000000-0000-4000-8000-000000000001 \
  --revision 1 --name "Service request volume" --spec-file chart.vl.json
```

Use the revision last returned by `get`, `create`, or `update`. A stale revision fails with a conflict; the CLI does not silently fetch a newer revision or overwrite someone else's edit. Supply at least one changed field. Omitted fields are preserved; `--clear-description` and `--clear-projects` clear their respective metadata, while repeated `--project-id` values replace the complete tag list.

Changing SQL text clears the previous result and related result metadata. Style/name/description/tag changes keep the result. Whitespace changes in SQL also count as a change. Fetch the latest definition, compute locally, then upload its result:

```sh
logcove charts result put chart_00000000-0000-4000-8000-000000000001 \
  --file result.json
logcove charts delete chart_00000000-0000-4000-8000-000000000001
```

Result replacement has no CLI revision parameter, result history, or background computation. Coordinate edits if several clients compute the same Chart: an upload is not tied to a client-supplied SQL revision. Delete removes the Chart and attempts to delete its current result object under the service's existing cleanup behavior; the CLI returns `{"data":{"id":"...","deleted":true}}`.

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
