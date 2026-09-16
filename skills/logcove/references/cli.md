# CLI operations

Use this reference for Logcove operations. All commands assume a compatible `logcove` executable on PATH. Check command-specific `--help` when in doubt; do not invent commands such as `logcove query` or `logcove data describe`.

## Identity and environment

```sh
logcove --version
logcove config show
logcove whoami
logcove login
```

If the deployment is not configured, get its actual API origin from the user or existing project settings, then use `logcove config set-api-url <origin>`. `--api-url <origin>` overrides the target for one invocation; use it consistently throughout the task. Precedence is flag, `LOGCOVE_API_URL`, saved configuration.

Login uses a signed Bearer Session, not a JWT or Vector write key. The CLI stores it in the OS credential service, independently of the desktop app. Do not read the keychain, request a Session token from the user, put Session tokens in shell commands, or build Session Authorization headers. Log ingestion uses a separate write-key header as described in [ingestion.md](ingestion.md).

`logcove login --no-browser` prints a link and waits for browser approval. A localhost link requires the associated local web application. An expired or denied request needs a new login attempt, not repeated approval of the old link. Do not log out or change accounts merely to diagnose a download error.

## Discover and pull

```sh
logcove projects list
logcove projects get prj_00000000-0000-4000-8000-000000000001
logcove data pull prj_00000000-0000-4000-8000-000000000001 \
  --from 2026-09-01 --to 2026-09-02 --output ./analysis/logs
```

Replace example IDs and dates with discovered values and the selected range. `projects list` follows all pages and defaults to active Projects; `--status archived` or `--status all` includes archived metadata. `projects get` may describe an archived Project, which does not make its data downloadable. Management output includes `id`, `name`, `description`, `status`, `ingestion_protocol`, `data_prefix`, `write_key_id`, `ingestion.desired_revision`, and timestamps.

`data pull` handles pagination, signing, retries for expired access, concurrency, object validation, and manifest publication. A completed response is:

```json
{"data":{"project_id":"prj_00000000-0000-4000-8000-000000000001","manifest_path":"/your/output/pull-example/manifest.json","file_count":12,"total_bytes":42000}}
```

Each invocation creates a new run directory. Its manifest records `api_url`, `project_id`, `start_date`, `end_date`, `completed_at`, and `files`; each file has an object `key`, `size`, `etag`, `ingest_date`, `uploaded_at`, and relative `path`. The manifest contains no signed URLs. A retry starts a new run, not a resume of an incomplete one. With `--reuse-manifest`, matching files from completed runs are copied locally into the new run; the result remains self-contained. The response also reports `downloaded_file_count` and `reused_file_count`; `file_count` and `total_bytes` include both.

For more than 93 ingestion days, use separate bounded pulls only for the requested coverage and data still available from retention. Do not combine overlapping pulls of the same objects as extra events; see [duckdb.md](duckdb.md).

### Local reuse and freshness

Before a pull, inspect completed manifests in the task's known download directory and reuse their files when they satisfy the question. Match `api_url`, `project_id`, and the requested UTC ingestion dates; use only data known to belong to the intended account. Current manifests do not record account identity, so keep separate working directories for different accounts. Verify that listed files still exist with the recorded byte sizes; ignore incomplete runs and `.part` files. Do not identify cached objects by signed URLs or numeric local filenames: use the source, object `key`, `etag`, and `size` from the manifest.

For an existing snapshot, rerun DuckDB directly against its exact file list without signing or downloading it again. State the manifest's `completed_at` and coverage. For a refresh or wider period, pull the desired range with the previous completed manifests as reuse inputs:

```sh
logcove data pull prj_00000000-0000-4000-8000-000000000001 \
  --from 2026-09-01 --to 2026-09-03 --output ./analysis/logs \
  --reuse-manifest ./analysis/logs/pull-previous/manifest.json
```

Use real paths discovered locally, and repeat `--reuse-manifest` for additional completed runs of the same API and Project. The CLI checks the latest authorized file listing, matches key/ETag/size, and checks local size and Parquet markers. Only missing, changed, or invalid local files need signed download links and GET requests. Listing still makes API/storage requests. Existing files are copied rather than hard-linked: this avoids another network download, but uses local disk space. Keep existing local files; do not clear the download directory before each analysis. Query only the returned new manifest, which contains both reused and downloaded files; do not append old manifests to it.

A completed manifest is a snapshot of the listed files, not proof that a date partition will never receive more uploads. If the user asks for current data, refresh the relevant dates; do not skip a date merely because some files for it already exist. This includes delayed uploads to earlier ingestion dates. Do not describe an older snapshot as current.

Check `logcove data pull --help` for `--reuse-manifest`; the change is implemented in source but is not in the published v0.2.0 binary. Without this option, reusing `--output` alone does not skip existing files. On an older binary, reuse an adequate local snapshot directly, or explain that refreshing a previously downloaded date requires downloading that date again until the CLI is upgraded. Do not invent flags or bypass the CLI with authenticated API requests.

## Manage Projects and write keys

Each Project has one immutable `ingestion_protocol`: `http_json` (the default) or `otlp_http` (OTLP/HTTP Protobuf Logs). Choose it at creation:

```sh
logcove projects create --name "OTel logs" --ingestion-protocol otlp_http
```

Use `--ingestion-protocol http_json` for ordinary JSON. `projects update` does not accept the protocol; create a different Project to change formats. The same write Key may bind Projects of different protocols, but each request must use the matching Project and protocol endpoint. This flag requires the updated API; these source changes are not yet released. A created OTLP Project alone does not prove its collector endpoint is deployed. Use the endpoint supplied by that environment, never guess it from the API host.

These commands require the current management-capable CLI; the published v0.2.0 binary lacks them. Check `projects --help` and `keys --help`. They use the saved login Session and do not need DuckDB or data downloads.

```sh
logcove projects create --name "Backend logs" --description "HTTP requests"
logcove keys create --name "Backend collector" \
  --project-id prj_00000000-0000-4000-8000-000000000001 \
  --output /path/to/private/collector.key
logcove projects get prj_00000000-0000-4000-8000-000000000001
```

Use the returned Project ID. Choose a new file outside source control in an existing directory. `keys create` writes the raw credential plus a newline there, with `0600` permissions on Unix or a protected owner-only DACL on Windows. It returns masked metadata, the Key ID and an absolute `data.key_file`, never `data.key`. Do not cat, echo, include, or ask the user to paste the secret. A collector setup script may read the file internally without printing it. The CLI never overwrites an existing destination.

Commands for subsequent management:

```sh
logcove projects list --status all
logcove projects update prj_00000000-0000-4000-8000-000000000001 --name "Renamed logs"
logcove projects update prj_00000000-0000-4000-8000-000000000001 --clear-description
logcove projects update prj_00000000-0000-4000-8000-000000000001 --write-key-id key_00000000-0000-4000-8000-000000000001
logcove projects update prj_00000000-0000-4000-8000-000000000001 --clear-write-key
logcove projects archive prj_00000000-0000-4000-8000-000000000001
logcove projects restore prj_00000000-0000-4000-8000-000000000001
logcove keys list --status active
logcove keys get key_00000000-0000-4000-8000-000000000001
logcove keys update key_00000000-0000-4000-8000-000000000001 --name "Renamed collector"
logcove keys set-projects key_00000000-0000-4000-8000-000000000001 --project-id prj_00000000-0000-4000-8000-000000000001
logcove keys set-projects key_00000000-0000-4000-8000-000000000001 --clear-projects
logcove keys revoke key_00000000-0000-4000-8000-000000000001
```

These are independent examples, not a setup script to run in sequence. Only perform mutations covered by the user's request. Archive preserves logs but disables reads and ingestion; restore reverses that status. There is no hard-delete Project command. Revocation is irreversible, unbinds every Project, and preserves masked metadata; it does not delete local credential files.

One Project can have one write key. Key creation and `keys set-projects` reject Projects already using another Key. `set-projects` replaces the **entire** set: supply every intended Project with repeated `--project-id`, or explicitly clear it. For intentional single-Project replacement, use `projects update --write-key-id` with the Key resource ID. Names/descriptions and a binding change can share one Project update.

Management success means the desired state was saved. Vector applies ingestion changes asynchronously; `ingestion.desired_revision` is not a loaded-state acknowledgement. Do not claim the collector has already accepted the new key solely from management success.

If local file reservation fails (`KEY_FILE_ERROR`), no creation request was sent. If creation's response is lost, inspect `keys list` before retrying; the API cannot recover the secret. If saving fails after creation (`KEY_FILE_WRITE_FAILED`), the error provides the created Key ID and a revoke command. Explain the partial outcome and handle that Key within the user's authorized scope before retrying. Never print API response bodies or secret files to diagnose it.

## Output and recovery

Successful operations return JSON on stdout, usually under `data`. Help/version output is plain text. Progress, browser instructions, warnings, and runtime errors use stderr. Inspect the exit status before parsing stdout; stderr is not a single JSON document when progress messages precede an error. Quote paths, or prefer subprocess argument arrays when generating commands.

| Failure | Next action |
| --- | --- |
| `UNAUTHENTICATED` | Authorize through `logcove login` for the same API environment |
| Credential-store errors | Report the OS-store requirement; Linux needs an unlocked Secret Service. Do not fall back to plaintext storage |
| `ACCESS_DENIED`, `NOT_FOUND` | Check the selected account, resource, and environment; do not restart login automatically |
| `NETWORK_ERROR`, `SERVER_ERROR`, `RATE_LIMITED` | Report the failure and request ID if present; retry reads only when appropriate, without an unbounded loop |
| `OBJECT_CHANGED`, `DOWNLOAD_FAILED`, `DOWNLOAD_DENIED` | Do not analyze the incomplete pull; a new pull starts fresh, and `--concurrency 1` may help diagnose connection pressure |
| `INVALID_CACHE` | Check the supplied completed manifest, API origin and Project; do not edit its identity to force reuse |
| `PROJECT_LIMIT_REACHED` | The plan's active Project limit is full. Explain the limit; archiving another Project or upgrading requires the user's authorization. Retrying unchanged input will not help |
| `CHART_CONFLICT` | Read the latest Chart revision and reconcile the definition before retrying |
| `WRITE_KEY_CONFLICT`, `INVALID_KEY_BINDING`, `KEY_REVOKED` | Inspect current Project bindings and Key revocation. A revoked Key cannot be restored; choose an active Key or an explicit replacement within the user's scope |
| `CONFLICT` | An unrecognized conflict, or an older CLI. Inspect current state and the request ID before retrying; do not assume it is a Chart revision conflict |
| `INVALID_INPUT`, `PAYLOAD_TOO_LARGE` | Correct command/input files or reduce aggregate size; do not silently truncate rows |

A failed write may have reached the service before its response was lost. Inspect existing state before retrying Chart creation or other mutations, to avoid duplicates or overwrites. This Skill grants no additional permission to delete Charts, revoke sessions, or change configuration beyond the user's task.
