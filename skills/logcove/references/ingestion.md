# Connect and verify log ingestion

Use this reference when connecting an application or Collector, generating a "first log" example, or checking whether logs arrived. Use the CLI for Project/Key management and downloads; send logs directly to the environment's collector. There is no `logcove ingest` command.

## Project, endpoint, and identity

Inspect `logcove projects get <project-id>` before configuring a sender. The Project must be active and have a bound write key. For authorized creation or binding changes, follow [cli.md](cli.md#manage-projects-and-write-keys), including checking the installed CLI's available commands.

| Project `ingestion_protocol` | Sender | Request format |
| --- | --- | --- |
| `http_json` | cURL or Python requests, or the application's HTTP client | Ordinary JSON log object, `Content-Type: application/json` |
| `otlp_http` | Official OTel Logs SDK/exporter or OTel Collector | OTLP HTTP Protobuf Logs, `Content-Type: application/x-protobuf`; gzip is supported |

A Project accepts exactly one protocol, fixed at creation. A key may bind multiple Projects, but each request must use a matching Project and endpoint. Do not send ordinary JSON, OTLP JSON, gRPC, traces, or metrics to the Logcove OTLP Logs endpoint.

Get the actual complete ingestion URL from the selected environment's settings or the user. It is separate from the CLI API origin. Do not infer an OTLP URL from a JSON URL, substitute one when missing, or assume a planned production hostname is deployed. For OTLP, use the full Logs URL (normally ending in `/v1/logs`), without appending that path a second time. If the address is unknown, leave a protocol-specific placeholder and explain what must be configured before sending.

Both protocols require:

- `X-Project-ID`: the Project's full `prj_...` ID.
- `Authorization: Bearer <write-key>`: the raw credential from the private key file, not the `key_...` resource ID, masked key, or CLI Session.

`ingestion.desired_revision` only records the requested configuration. Project creation, key binding, rotation, and revocation reach the collector asynchronously. Do not claim immediate readiness from a successful management command.

## Read the key inside the sender

Use `data.key_file` returned by `logcove keys create --output <new-private-file>`, or an existing private file provided by the user. Do not print the file, include the secret in generated source/YAML, put it in process arguments, or ask the user to paste it into chat. Existing keys cannot be revealed again through the API. Only create or replace keys within the authorized setup scope.

For generated Python integrations, use non-secret environment settings `LOGCOVE_PROJECT_ID`, `LOGCOVE_INGESTION_URL`, and `LOGCOVE_WRITE_KEY_FILE`. Read the raw key file (a string plus newline, not JSON) inside the application:

```python
import os
from pathlib import Path

endpoint = os.environ["LOGCOVE_INGESTION_URL"]
write_key = Path(os.environ["LOGCOVE_WRITE_KEY_FILE"]).read_text().strip()
headers = {
    "X-Project-ID": os.environ["LOGCOVE_PROJECT_ID"],
    "Authorization": "Bearer " + write_key,
}
```

Pass `endpoint` and `headers` directly to the HTTP client or exporter without logging them. For cURL, supply the credential through standard input/config from a helper that reads the file, rather than expanding it into `-H` process arguments. Disable verbose request/header dumps when working with real credentials.

## HTTP JSON examples

Provide cURL for a manual quick start or Python requests for application integration. A single JSON object with `message`, `service`, and `level` is enough for a first log; these are sample application fields, not an assumed schema for every Project. Include a unique non-secret test marker so this request can be found later. With requests, use `json=...`, an explicit timeout, and check `raise_for_status()`; with cURL, check HTTP failure and the process exit status.

Preserve the application's event timestamp when needed. Do not set `_created_time` or try to choose a Project through fields in the payload: the collector controls receipt time and authenticated Project identity.

## OTLP Python / FastAPI

Use the official `opentelemetry-sdk` and `opentelemetry-exporter-otlp-proto-http` packages, reusing the application's existing OTel setup when present. A minimal FastAPI example needs these pieces:

- `LoggerProvider` with a `Resource` containing `service.name`.
- `OTLPLogExporter` from `opentelemetry.exporter.otlp.proto.http._log_exporter`, with the full `endpoint`, dictionary `headers` from the private file, optional `Compression.Gzip`, and an explicit timeout. Header dictionary values are literal: use `Bearer ` with a space, not `Bearer%20`.
- `BatchLogRecordProcessor` and a `LoggingHandler` connected to a Python logger at an appropriate level. Emit an actual log inside a route; receiving HTTP requests or enabling tracing alone does not create application log records.
- Flush/shutdown on application exit. For a standalone FastAPI lifespan, `await asyncio.to_thread(provider.shutdown)` after `yield` drains the provider without blocking the event loop. Do not add duplicate handlers/providers to an already instrumented application.

Only configuring exporter environment variables does not activate logging or capture all application logs. Explain the scope of the example: one explicit route log is not automatic capture of every framework/access log. For a short-lived script, emit the log and shut down the provider before exiting.

Python export timeouts are in seconds; `timeout=120` is a reasonable initial value for this ingestion flow, not a delivery SLA. Match timeout units to the actual SDK rather than copying a millisecond value from another language. Use bounded retries; a timeout can occur after acceptance, so resending may create duplicates.

## OTel Collector

For an existing Collector, merge the exporter into its existing Logs pipeline without replacing unrelated configuration. A minimal local Collector can use this configuration:

```yaml
receivers:
  otlp:
    protocols:
      http:
        endpoint: 127.0.0.1:4318
processors:
  batch: {}
exporters:
  otlp_http/logcove:
    logs_endpoint: ${env:LOGCOVE_INGESTION_URL}
    encoding: proto
    compression: gzip
    headers:
      X-Project-ID: ${env:LOGCOVE_PROJECT_ID}
      Authorization: "Bearer ${env:LOGCOVE_WRITE_KEY}"
    timeout: 120s
service:
  pipelines:
    logs:
      receivers: [otlp]
      processors: [batch]
      exporters: [otlp_http/logcove]
```

The Collector process needs `LOGCOVE_WRITE_KEY` in its environment. A launcher can read `LOGCOVE_WRITE_KEY_FILE` internally and set that child-process environment value without printing it or putting it in the command line. The Collector does not interpret the key-file variable itself. Keep the YAML free of literal secrets.

Use the exporter name supported by the installed distribution: current Collector uses `otlp_http`; older versions use `otlphttp`. Validate with `otelcol validate --config otelcol.yaml`, then run `otelcol --config otelcol.yaml`; use `otelcol-contrib` if that is the installed distribution. See the [official installation guide](https://opentelemetry.io/docs/collector/installation/).

An OTLP JSON test request may be sent to this local Collector at `http://127.0.0.1:4318/v1/logs`. The Collector converts it to Protobuf before forwarding to Logcove. Make that distinction explicit; ordinary `{"message":"..."}` JSON is not an OTLP JSON envelope. The loopback receiver is for local applications; remote or containerized applications need an explicitly reachable receiver configuration.

## Verify arrival, not just acceptance

Send a small marked log within the user's requested scope; do not turn a setup request into a load test. Check the application's exporter outcome (including any OTLP partial-success rejection), not just the FastAPI route's HTTP 200. A local Collector's successful receive response also does not prove its downstream export succeeded.

Separate these observations in the report: management saved; collector accepted; uploaded Parquet contains the test record. The target batch timeout is 90 seconds; environments still using older configurations may differ, size limits may flush earlier, and buffering or retries may delay visibility. There is no guarantee of visibility at exactly 90 seconds. Wait for a batch upload, then use `logcove data pull` for the UTC receipt date and inspect its completed manifest with DuckDB as described in [duckdb.md](duckdb.md). Include both dates if the test crosses UTC midnight. Search for the unique marker and check relevant fields; do not infer receipt from a nonempty Project alone. An empty manifest means no files are available for that pull yet, not proof that the sender lost the event.

For the web UI, click Preview **Refresh** after waiting: cached results do not automatically fetch new logs. Stop validation after a bounded wait and report the last verified stage if data remains unavailable. Do not continuously resend the test log or promise exactly-once delivery; request retries can create duplicate records.
