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

Login uses a signed Bearer Session, not a JWT or Vector write key. The CLI stores it in the OS credential service, independently of the desktop app. Do not read the keychain, request a token from the user, put tokens in shell commands, or build Authorization headers.

`logcove login --no-browser` prints a link and waits for browser approval. A localhost link requires the associated local web application. An expired or denied request needs a new login attempt, not repeated approval of the old link. Do not log out or change accounts merely to diagnose a download error.

## Discover and pull

```sh
logcove projects list
logcove projects get prj_00000000-0000-4000-8000-000000000001
logcove data pull prj_00000000-0000-4000-8000-000000000001 \
  --from 2026-09-01 --to 2026-09-02 --output ./analysis/logs
```

Replace example IDs and dates with discovered values and the selected range. `projects list` already follows all pages and returns active readable Projects. `projects get` may describe an archived Project, which does not make its data downloadable. Metadata includes `id`, `name`, `description`, `status`, `data_prefix`, and timestamps.

`data pull` handles pagination, signing, retries for expired access, concurrency, object validation, and manifest publication. A completed response is:

```json
{"data":{"project_id":"prj_00000000-0000-4000-8000-000000000001","manifest_path":"/your/output/pull-example/manifest.json","file_count":12,"total_bytes":42000}}
```

Each invocation creates a new run directory. Its manifest records `api_url`, `project_id`, `start_date`, `end_date`, `completed_at`, and `files`; each file has an object `key`, `size`, `etag`, `ingest_date`, `uploaded_at`, and relative `path`. The manifest contains no signed URLs. A retry starts a new run, not a resume of the previous one.

For more than 93 ingestion days, use separate bounded pulls only for the requested coverage and data still available from retention. Do not combine overlapping pulls of the same objects as extra events; see [duckdb.md](duckdb.md).

## Output and recovery

Successful operations return JSON on stdout, usually under `data`. Help/version output is plain text. Progress, browser instructions, warnings, and runtime errors use stderr. Inspect the exit status before parsing stdout; stderr is not a single JSON document when progress messages precede an error. Quote paths, or prefer subprocess argument arrays when generating commands.

| Failure | Next action |
| --- | --- |
| `UNAUTHENTICATED` | Authorize through `logcove login` for the same API environment |
| Credential-store errors | Report the OS-store requirement; Linux needs an unlocked Secret Service. Do not fall back to plaintext storage |
| `ACCESS_DENIED`, `NOT_FOUND` | Check the selected account, resource, and environment; do not restart login automatically |
| `NETWORK_ERROR`, `SERVER_ERROR`, `RATE_LIMITED` | Report the failure and request ID if present; retry reads only when appropriate, without an unbounded loop |
| `OBJECT_CHANGED`, `DOWNLOAD_FAILED`, `DOWNLOAD_DENIED` | Do not analyze the incomplete pull; a new pull starts fresh, and `--concurrency 1` may help diagnose connection pressure |
| `CONFLICT` | Read the current Chart, reconcile the intended edit, then use its revision |
| `INVALID_INPUT`, `PAYLOAD_TOO_LARGE` | Correct command/input files or reduce aggregate size; do not silently truncate rows |

A failed write may have reached the service before its response was lost. Inspect existing state before retrying Chart creation or other mutations, to avoid duplicates or overwrites. This Skill grants no additional permission to delete Charts, revoke sessions, or change configuration beyond the user's task.
