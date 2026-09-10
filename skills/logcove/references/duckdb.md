# Local DuckDB analysis

DuckDB is an external tool, not part of the Logcove CLI. Use an already available DuckDB environment. The examples use Python's `duckdb` package and standard-library JSON; no pandas dependency is needed.

## Register downloaded sources

Use one chosen completed manifest per Project for a normal analysis. Confirm its API origin and ingestion dates match the task. Resolve the exact listed paths, including on Windows, rather than constructing a Parquet glob:

```python
import json
from pathlib import Path
import duckdb

manifest_path = Path("analysis/logs/pull-example/manifest.json")
manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
paths = [str(manifest_path.parent / item["path"]) for item in manifest["files"]]
if not paths:
    raise SystemExit("No files in the selected ingestion dates")

# Select the requested UTC bounds; keep them outside saved Chart SQL.
start = "2026-09-03T00:00:00Z"
end = "2026-09-04T00:00:00Z"
literal = lambda value: "'" + value.replace("'", "''") + "'"
db = duckdb.connect()
db.execute("SET TimeZone='UTC'")
source = db.read_parquet(paths, union_by_name=True)
if "_created_time" not in source.columns:
    raise ValueError("Historical source data needs _created_time migration")
invalid = source.filter('_created_time IS NULL OR NOT isfinite(_created_time::TIMESTAMPTZ)').count('*').fetchone()[0]
if invalid:
    raise ValueError("Missing or invalid _created_time; repair source data")
source.project('* REPLACE (_created_time::TIMESTAMPTZ AS _created_time)').filter(
    f"_created_time >= {literal(start)}::TIMESTAMPTZ AND _created_time < {literal(end)}::TIMESTAMPTZ"
).create_view(manifest["project_id"])
print(db.sql('DESCRIBE "' + manifest["project_id"] + '"').fetchall())
```

For multiple Projects, register their manifests in the **same connection**, using each full Project ID as its view name. A separate in-memory DuckDB connection or process does not retain these views. SQL identifiers containing the ID's hyphens require double quotes.

When recalculating a saved Chart, use its `project_ids` to identify the sources to download or reuse, authorize each read, then register one view per ID before executing the saved SQL. The CLI and API do not initialize these views automatically. If the declared dependencies do not match the SQL (including an older Chart with an empty list but Project references in SQL), reconcile the definition first; do not silently skip missing sources or assume an empty list means no logs. New analysis must save the complete source list alongside its SQL.

When combining multiple pulls for one Project, combine the chosen files and register one view. Deduplicate repeated object keys from overlapping ranges or reruns. If the same key has different ETags, select a consistent pull instead of silently mixing versions. This removes repeated file reads, not duplicate log events: event-level deduplication depends on the dataset's actual semantics.

`union_by_name=True` accommodates missing columns across Parquet files but does not guarantee compatibility for all type changes. Inspect conflicting files and cast deliberately if needed. An empty file list has no inferred schema; do not call `read_parquet([])` or present that as a successful zero-row aggregate.

## Inspect and compute

Start with column names/types and a small sample of fields relevant to the question. `_created_time` is reserved for the Vector receipt timestamp, and views normalize it to TIMESTAMPTZ. Historical files without it or mixed files producing NULL system times need repair before time-filtered analysis. Business log formats are not fixed. A timestamp might be a string, timestamp, or integer epoch; determine its timezone and unit before filtering. If `try_cast` is useful, also count failed conversions instead of silently treating them as missing events.

Downloads select UTC ingestion dates, while the user's period may describe event time in another timezone. Use an explicit half-open event-time interval in SQL when the question needs it, and explain whether the selected ingestion partitions cover possible late arrivals. Historical snapshots are not a live stream.

After registering the filtered view and confirming `service` exists, a reusable Chart query could be:

```sql
SELECT service, count(*) AS requests
FROM "prj_00000000-0000-4000-8000-000000000001"
GROUP BY service
ORDER BY requests DESC, service;
```

Save the actual query to `query.sql`. Keep file loading/view registration outside it so the saved Chart SQL can be reused on another machine. Replace the example ID and fields with the discovered source. For cross-source joins, inspect uniqueness and join coverage before interpreting counts; the dependency list does not define join keys, and sources need not use the same request ID.

Check row counts and totals against the question. Distinguish no matching events from missing data or failed parsing. For ratios, state the denominator and handle zero denominators explicitly. Describe aggregation granularity, timezone, coverage, and relevant data-quality limitations with the result.

If the machine is constrained, narrow the requested range or select relevant columns and aggregate in DuckDB. DuckDB memory/thread limits and a local spill directory can be configured for the available environment; do not automatically consume all cores or claim browser/container fallback is implemented.

## Verify and render locally

Run this in the same connection where the time-filtered Project views were registered. Use the actual requested bounds when registering views, outside the saved SQL. Local result files are optional working artifacts and are never uploaded to the Chart API:

```python
from datetime import datetime, timezone

query_path = Path("query.sql")
cursor = db.execute(query_path.read_text(encoding="utf-8-sig"))
columns = [column[0] for column in cursor.description]
if len(columns) != len(set(columns)):
    raise ValueError("Use unique SQL aliases for result columns")
values = cursor.fetchmany(10001)
if len(values) > 10000:
    raise ValueError("Aggregate further: The Chart UI supports at most 10000 rows")
rows = [dict(zip(columns, row)) for row in values]
data_json = json.dumps(rows, ensure_ascii=False, allow_nan=False, separators=(",", ":"))
if len(data_json.encode("utf-8")) > 5 * 1024 * 1024:
    raise ValueError("Aggregate further: Chart result data exceeds 5 MiB")
result = {
    "computed_at": datetime.now(timezone.utc).isoformat(timespec="milliseconds"),
    "data": rows,
}
Path("result.json").write_text(
    json.dumps(result, ensure_ascii=False, allow_nan=False, separators=(",", ":")),
    encoding="utf-8",
)
```

This example intentionally expects JSON-compatible query output. Cast or convert date/time, decimal, binary, and other non-JSON values according to their meaning before serialization. Give temporal values an explicit timezone where relevant. Do not use a blanket `default=str`, silently convert NaN/Infinity to strings, or discard rows to pass limits. JavaScript chart rendering cannot represent integers above 2^53-1 exactly; preserve large identifiers as strings.

An empty result array is valid when an actual query over known, available data produces no matching rows. That differs from an empty download manifest. Keep raw Parquet and investigation samples local; save only the Chart definition to the service.
