# Analysis Skill validation

Current release (2026-10-03): the matching online-service Skill is published with
CLI v0.3.1 and as a standalone ZIP. Skill/DuckDB and five-platform release checks
passed; downloaded archives match their published checksums. See [release verification](releases.md).
The dated instruction-review and rendering evidence below is historical, not a
new acceptance run for v0.3.1 or a claim of fresh agent-host verification.

Date: 2026-09-08. Scope: the instruction-only `skills/logcove` package and its CLI/DuckDB/Vega-Lite workflow. This is source-checkout validation, not a binary release or a hosted deployment.

## Verified

| Check | Result |
| --- | --- |
| Skill frontmatter, naming, and scaffold validation | Passed using the Skill Creator validator |
| Standalone copy of the whole Skill directory | All local reference links resolve without the repository or private backend source |
| Python examples executed from the reference text | Passed with DuckDB 1.5.5 |
| Multiple Parquet files with an added column | All six synthetic rows preserved using `union_by_name` |
| Known aggregate | Exact service counts matched the six-row fixture |
| Two Project views in one DuckDB connection | Cross-source join returned the expected six rows |
| Duplicate output column names | Rejected before constructing ambiguous result objects |
| More than 10,000 result rows or 5 MiB of data | Rejected rather than silently truncated |
| Wide result near the data size limit | 10,000 rows with 66 columns produce 5,200,001 bytes of compact data and a 5,200,056-byte result file; the actual CLI result validator accepts it |
| Nonfinite JSON number | Rejected by explicit serialization |
| Empty query result over known data | Accepted as an empty result array |
| Empty download manifest | Stopped before inferring a schema or producing replacement results |
| Real CLI identity, Project discovery, and Parquet download | Passed against the local development API and real R2: 56 files, 3,284,233 bytes |
| Real aggregate using the reference SQL/result examples | Four service counts sum to 100,000 records and match the known synthetic dataset |
| Temporary Chart create/get | Saved SQL, spec, and result rows matched the local artifacts |
| Metadata update | Existing result preserved |
| SQL text update | Existing result cleared |
| Recompute and result replacement | Current SQL checked before upload; retrieved rows matched |
| Rendering the retrieved spec and rows | Vega-Lite 6.4.3 / Vega 6.4.0 produced SVG with four bars and no compile warnings |
| Test cleanup | Temporary Chart deleted; local artifacts retained only under ignored `experiments/` |

The shared CLI login and existing useful Charts were preserved. The live workflow used only CLI service operations; reading and executing the bundled examples did not require an API key, MCP server, or direct database access. The validator's PyYAML dependency ran in an isolated tooling environment; no dependency was added to the CLI or Skill package.

## Instruction review

Reviewed the instructions against the current CLI and established product scope:

- Service actions use the CLI; Session secrets and Vector write keys are not part of analysis prompts.
- Project IDs are source metadata, and a Chart may refer to multiple Projects.
- Local-only tasks do not imply uploading data or saving Charts. Existing user authorization is reused for requested saves.
- Viewing Charts and changing only metadata/style explicitly skip log downloads, SQL execution, and result replacement. New analysis or recalculation can reuse suitable completed manifests.
- UTC ingestion partitions are distinguished from event-time predicates and late-arrival coverage.
- Analysis reads completed manifest file lists and handles overlapping downloads without counting the same objects twice.
- Saved SQL uses full-Project-ID views rather than local paths; spec data stays in the named `result` dataset.
- Definition conflicts are reconciled; result replacement does not claim an atomic SQL-revision precondition.
- Retrieved log/metadata text is treated as data, including text that resembles instructions.
- No scheduled calculation, thumbnail, `query_params`, container authentication, or CLI query engine is introduced.

These are instruction-review outcomes, not measured success rates from independent model runs.

## Boundaries and next step

Codex and Claude Code installation paths and invocation syntax were checked against their official documentation, linked from [skills.md](skills.md). The installed-copy check validates package portability; it does not establish successful automatic discovery or end-to-end behavior in a fresh session of either host. No independent Claude Code run, Windows runtime test, or native Linux credential-store test is claimed.

The render check used the Vega runtime directly to create SVG; it was not a new browser UI test. The existing CLI verification remains in [validation.md](validation.md). CLI code and CI workflows were unchanged for this Skill step, so the Rust suite was not rerun merely for instruction/documentation edits.

The next step at the time of this Skill review was CLI/Skill CI/CD. The five-platform
build and tag-triggered release pipeline has since been implemented and run for
v0.3.1. Fresh agent-host discovery, OS credential-store behavior and package-manager
installation retain their separate verification boundaries.
