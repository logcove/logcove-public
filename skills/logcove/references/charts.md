# Vega-Lite and saved Charts

Charts store definitions only: name, description, Project dependencies, SQL, and Vega-Lite spec. Calculation results stay local. There is no result upload endpoint, result history, thumbnail, or scheduled computation.

## Query and local verification

Use `project_ids` as the complete set of Project sources required by the SQL. Register each as a DuckDB view using its full Project ID. The list does not grant access; authorize reads through the CLI. Save a single SELECT query (including WITH/CTEs); keep setup statements such as SET or CREATE VIEW outside the saved SQL. Run the parser check in [duckdb.md](duckdb.md#verify-and-render-locally) before executing and saving it.

The application registers each Project as a view filtered by system `_created_time` in the selected half-open range `[start, end)`, normalized to TIMESTAMPTZ. Saved SQL only queries those views and computes the metric; do not include `$start_time`, `$end_time`, or other parameters. Cross-source queries see the same selected receipt-time range for every source. Do not hardcode the current window. Existing `$start_time` / `$end_time` bindings remain compatible, but their predicates are additional conditions inside the filtered views. When updating a legacy definition, explicitly remove obsolete selection predicates; the application does not rewrite SQL. Business event-time fields remain ordinary data and may differ from receipt time. See [duckdb.md](duckdb.md) to reproduce the filtered views locally.

The application selects files by UTC ingestion dates, then executes the SQL event-time predicates. It does not automatically discover late-arriving events stored in other ingestion partitions. A short time range can still require downloading a whole day's files. Missing source files provide no schema: report this instead of fabricating a zero count. Existing Charts may have incomplete dependencies or fixed dates; reconcile those definitions before recalculating.

When the task requires analysis, compute locally to verify the query and spec, but do not upload results. For metadata-only edits, retrieve and update the definition without downloading or recalculating data.

## Vega-Lite specification

Use root `data` exactly `{"name":"result"}`:

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

Adapt fields and encodings to the actual aggregate output. Do not embed inline values, top-level datasets, external URLs, or other named datasets in the saved spec. A local renderer injects the computed rows into `result`; the webpage/desktop app does this after its own local calculation. SQL/spec are limited to 64 KiB / 128 KiB. The application displays at most 10,000 aggregate rows and 5 MiB of result JSON; reduce the SQL output rather than silently truncating it. Cast identifiers exceeding JavaScript's safe integer range to VARCHAR.

Use an available Vega-Lite renderer to check the chart when appropriate; JSON parsing or successful definition storage alone does not prove rendering. The web/desktop app exports PNG/SVG after calculation. The CLI has no image export command.

## Save and update definitions

When saving is within the user's requested scope:

```sh
logcove charts create --name "Requests by service" \
  --description "Requests grouped by service within the selected time window" \
  --project-id prj_00000000-0000-4000-8000-000000000001 \
  --sql-file query.sql --spec-file chart.vl.json
logcove charts get chart_00000000-0000-4000-8000-000000000001
```

Use the actual returned Chart ID for `get`. Supply every Project with repeated `--project-id`; only omit it when the SQL has no Project sources. The CLI explicitly sends an empty dependency list in that case. Creation stores no result and starts no computation on the server.

For an existing Chart, read its definition and revision first:

```sh
logcove charts update chart_00000000-0000-4000-8000-000000000001 \
  --revision 1 --sql-file query.sql \
  --project-id prj_00000000-0000-4000-8000-000000000001
```

Use the observed revision, not the sample `1`. Every SQL update also requires the complete Project list, or `--clear-projects` for SQL without Project sources. Omitted fields are preserved. `--clear-description` clears the description. A revision conflict requires a fresh read and reconciliation; do not blindly overwrite.

Verify saved SQL, dependencies and spec by reading them back. Return the ID and the actual known web URL `/charts/<id>`; do not derive a web origin by stripping `api` from a hostname. Opening a Chart calculates its selected time range locally when there is no cached result. The default is the last hour, so historical data may require choosing a different range. Cache survives navigation in the same application instance; Refresh recomputes it. Deletion removes only the definition and requires user-requested deletion scope:

```sh
logcove charts delete chart_00000000-0000-4000-8000-000000000001
```
