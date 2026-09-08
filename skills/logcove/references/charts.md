# Vega-Lite and saved Charts

Use this reference when generating chart artifacts or saving them to Logcove. Charts are user-level resources; Project IDs are optional source tags, not ownership or access boundaries.

## Specification and result

Use a Vega-Lite spec with root `data` exactly `{"name":"result"}`. For example, for a computed result containing `service` and `requests`:

```json
{
  "$schema": "https://vega.github.io/schema/vega-lite/v6.json",
  "data": {"name": "result"},
  "mark": "bar",
  "encoding": {
    "x": {"field": "service", "type": "nominal", "sort": "-y"},
    "y": {"field": "requests", "type": "quantitative", "title": "Requests"},
    "tooltip": [
      {"field": "service", "type": "nominal"},
      {"field": "requests", "type": "quantitative"}
    ]
  }
}
```

Adapt encodings to the actual result fields/types. Prefer readable units, explicit temporal granularity, and labels in the user's language. Child layers may inherit the root dataset. Do not embed inline values, top-level datasets, external URLs, or unrelated named datasets in a saved spec; a local preview should inject rows into `result` at render time instead of changing the saved data declaration.

`result.json` has exactly two fields:

```json
{"computed_at":"2026-09-03T00:00:00Z","data":[{"service":"api","requests":12}]}
```

Generate `computed_at` at computation time; do not reuse the sample timestamp. The CLI normalizes RFC 3339 timestamps to UTC milliseconds. The limits are 64 KiB for SQL, 128 KiB for the spec input file, and 10,000 object rows / 5 MiB of result data. UTF-8 files may have a leading BOM. Follow [duckdb.md](duckdb.md) for result serialization.

Check that referenced fields exist and the aggregate answers the question. Use an available compatible Vega-Lite/Vega renderer or the web UI to check a rendered chart when possible. JSON parsing or successful API storage alone does not prove rendering works; report that limit if no renderer is available. Export images through an available renderer; the CLI does not have a chart-image export command.

## Save a new Chart

When saving is within the user's requested scope, submit the definition and initial result together:

```sh
logcove charts create --name "Requests by service" \
  --description "Requests in the selected ingestion dates" \
  --project-id prj_00000000-0000-4000-8000-000000000001 \
  --sql-file query.sql --spec-file chart.vl.json --result-file result.json
logcove charts get chart_00000000-0000-4000-8000-000000000001
```

Use the ID returned by `create` for `get`; all IDs above are examples. Repeat `--project-id` for multiple source tags. Description, tags, and initial result are optional, but omitting a result does not trigger calculation. Record the meaningful date range and timezone in the description and/or SQL; there is no `query_params` field.

Verify the saved SQL/spec, result rows, and timestamps. Return the ID and, if known, the actual web-origin URL `/charts/<id>`. Do not derive the web origin by stripping `api` from a hostname.

## Update an existing Chart

```sh
logcove charts get chart_00000000-0000-4000-8000-000000000001
logcove charts update chart_00000000-0000-4000-8000-000000000001 \
  --revision 1 --name "Service request volume" --spec-file chart.vl.json
```

Use the current revision returned by the CLI, not the example `1`. Supply only intended changes. Omitted fields are preserved. `--clear-description` and `--clear-projects` explicitly clear metadata; repeated `--project-id` values replace the whole tag set. `--sql-file` changes the saved query.

Changing any SQL text, including whitespace, clears the saved result and its metadata. Name, style, description, and tag changes retain it. For a query change, compute using the intended SQL, update the definition with its observed revision, then upload the corresponding result:

```sh
logcove charts result put chart_00000000-0000-4000-8000-000000000001 --file result.json
logcove charts get chart_00000000-0000-4000-8000-000000000001
```

A definition conflict needs a fresh read and reconciliation; do not automatically retry with the newest revision. Result replacement has no revision argument or atomic SQL-match check. Before uploading a result to an existing Chart, re-read its current SQL and compare with what was executed. If it changed, stop that upload and reconcile. This catches observed changes but cannot prevent a simultaneous edit after the check; report conflicting activity if encountered rather than claiming atomic protection.

There is only a latest result: no result history, thumbnail, or scheduled computation. `charts get` reads stored data and never executes SQL. Only delete a Chart when deletion is part of the user's request:

```sh
logcove charts delete chart_00000000-0000-4000-8000-000000000001
```
