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

db = duckdb.connect()
db.read_parquet(paths, union_by_name=True).create_view(manifest["project_id"])
print(db.sql('DESCRIBE "' + manifest["project_id"] + '"').fetchall())
```

For multiple Projects, register their manifests in the **same connection**, using each full Project ID as its view name. A separate in-memory DuckDB connection or process does not retain these views. SQL identifiers containing the ID's hyphens require double quotes.

When combining multiple pulls for one Project, combine the chosen files and register one view. Deduplicate repeated object keys from overlapping ranges or reruns. If the same key has different ETags, select a consistent pull instead of silently mixing versions. This removes repeated file reads, not duplicate log events: event-level deduplication depends on the dataset's actual semantics.

`union_by_name=True` accommodates missing columns across Parquet files but does not guarantee compatibility for all type changes. Inspect conflicting files and cast deliberately if needed. An empty file list has no inferred schema; do not call `read_parquet([])` or use that alone to replace a saved result with zero rows.

## Inspect and compute

Start with column names/types and a small sample of fields relevant to the question. Log formats are not fixed. A timestamp might be a string, timestamp, or integer epoch; determine its timezone and unit before filtering. If `try_cast` is useful, also count failed conversions instead of silently treating them as missing events.

Downloads select UTC ingestion dates, while the user's period may describe event time in another timezone. Use an explicit half-open event-time interval in SQL when the question needs it, and explain whether the selected ingestion partitions cover possible late arrivals. Historical snapshots are not a live stream.

After confirming a `service` column exists, a reusable query could be:

```sql
SELECT service, count(*) AS requests
FROM "prj_00000000-0000-4000-8000-000000000001"
GROUP BY service
ORDER BY requests DESC, service;
```

Save the actual query to `query.sql`. Keep file loading/view registration outside it so the saved Chart SQL can be reused on another machine. Replace the example ID and fields with the discovered source. For cross-source joins, inspect uniqueness and join coverage before interpreting counts; do not join on Project tags or assume that every source uses the same request ID.

Check row counts and totals against the question. Distinguish no matching events from missing data or failed parsing. For ratios, state the denominator and handle zero denominators explicitly. Describe aggregation granularity, timezone, coverage, and relevant data-quality limitations with the result.

If the machine is constrained, narrow the requested range or select relevant columns and aggregate in DuckDB. DuckDB memory/thread limits and a local spill directory can be configured for the available environment; do not automatically consume all cores or claim browser/container fallback is implemented.

## Produce a Chart result file

Run this in the same connection where the Project views were registered:

```python
from datetime import datetime, timezone

query_path = Path("query.sql")
cursor = db.execute(query_path.read_text(encoding="utf-8-sig"))
columns = [column[0] for column in cursor.description]
if len(columns) != len(set(columns)):
    raise ValueError("Use unique SQL aliases for result columns")
values = cursor.fetchmany(10001)
if len(values) > 10000:
    raise ValueError("Aggregate further: Chart results allow at most 10000 rows")
rows = [dict(zip(columns, row)) for row in values]
data_json = json.dumps(rows, ensure_ascii=False, allow_nan=False, separators=(",", ":"))
if len(data_json.encode("utf-8")) > 5 * 1024 * 1024:
    raise ValueError("Aggregate further: Chart result data exceeds 5 MiB")
result = {
    "computed_at": datetime.now(timezone.utc).isoformat(timespec="milliseconds"),
    "data": rows,
}
Path("result.json").write_text(
    json.dumps(result, ensure_ascii=False, allow_nan=False), encoding="utf-8"
)
```

This example intentionally expects JSON-compatible query output. Cast or convert date/time, decimal, binary, and other non-JSON values according to their meaning before serialization. Give temporal values an explicit timezone where relevant. Do not use a blanket `default=str`, silently convert NaN/Infinity to strings, or discard rows to pass limits. JavaScript chart rendering cannot represent integers above 2^53-1 exactly; preserve large identifiers as strings.

An empty result array is valid when an actual query over known, available data produces no matching rows. That differs from an empty download manifest. Keep raw Parquet and investigation samples local; upload only the Chart data the user intends to share.
