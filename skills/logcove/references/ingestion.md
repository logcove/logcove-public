# Logcove log ingestion

Use the CLI for Project and write-key management. Send logs directly to the Logcove ingestion service; there is no `logcove ingest` command.

## Supported protocols

| Project `ingestion_protocol` | Endpoint | Accepted format |
| --- | --- | --- |
| `http_json` | `https://ingest.logcove.com/logs` | JSON log objects, `Content-Type: application/json` |
| `otlp_http` | `https://ingest.logcove.com/v1/logs` | OpenTelemetry Logs over OTLP/HTTP Protobuf, `Content-Type: application/x-protobuf`; gzip is supported |

A Project accepts exactly one protocol, fixed at creation. The OTLP endpoint supports Logs over HTTP Protobuf; it does not accept OTLP JSON, gRPC, traces or metrics. The user's OpenTelemetry integration remains their choice; this reference defines only Logcove's protocol and authentication contract.

## Project and authentication

Inspect `logcove projects get <project-id>`. The Project must be active and have a bound write key. For creation or binding changes, see [cli.md](cli.md#manage-projects-and-write-keys).

Both endpoints require:

- `X-Project-ID`: the Project's full `prj_...` ID.
- `Authorization: Bearer <write-key>`: the raw write key, not the `key_...` resource ID, masked key, login Session or personal access token.

Use the private file returned as `data.key_file` by `logcove keys create --output <new-private-file>`, or the user's existing secret configuration. Read credentials internally without exposing them in conversation, source code or command arguments. Existing keys cannot be revealed again through the API.

Project and key changes reach the ingestion service asynchronously. A successful management command does not prove that a new binding is already active. `_created_time` and authenticated Project identity are assigned by Logcove; preserve any separate business event timestamp in the log data.

## Verify stored logs when requested

Ingestion is batched, so accepted logs may not be immediately downloadable. To verify arrival, use `logcove data pull` for the relevant UTC receipt dates and query its completed manifest with DuckDB as described in [duckdb.md](duckdb.md). Check for the expected records; an accepted request alone does not prove they are stored, and an empty manifest does not prove data loss.

In the web app, Preview uses cached results until the user clicks **Refresh**. Report the last verified stage if records are not yet available; do not repeatedly resend logs or promise exactly-once delivery.
