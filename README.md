# Logcove CLI and Skills

Use your own computer to analyze Logcove logs and create charts with your coding agent.

This public repository contains the Rust CLI, agent Skills, and user documentation. The CLI talks only to Logcove's public user APIs. Log collection, the web application, and service deployment are maintained separately.

## Development status

The implementation plan is in [docs/implementation-plan.md](docs/implementation-plan.md). The CLI implements configuration, browser authorization, persistent sessions, Project discovery, concurrent Parquet downloads, and Chart operations. The shared analysis Skill is available from this checkout for Codex and Claude Code. See [CLI validation](docs/validation.md) and [Skill validation](docs/skill-validation.md) for actual verification boundaries. Automated distribution and prebuilt binary releases remain pending.

The CLI does not embed DuckDB or call an LLM. Agents use DuckDB directly for local computation and use the CLI for Logcove operations.

## Try the CLI

```sh
cargo build --locked
./target/debug/logcove config set-api-url http://localhost:8787
./target/debug/logcove login
./target/debug/logcove whoami
./target/debug/logcove projects list
```

After choosing a Project, use `logcove data pull` to download its Parquet files and a manifest. Analyze the manifest's files with DuckDB, then save SQL, a Vega-Lite specification, and aggregate results with `logcove charts create`. See the [download and Chart walkthrough](docs/cli.md#download-parquet).

The localhost example requires a running Logcove development API and web application. For a hosted deployment, configure its actual HTTPS API origin. See [CLI usage](docs/cli.md) for installation, environment selection, authentication, and credential-store requirements.

## Use with a coding agent

Install the whole [`skills/logcove`](skills/logcove/SKILL.md) folder for your agent using [these instructions](docs/skills.md), then ask it to analyze a selected source and period. For example: `Use $logcove to analyze requests by service for the last complete UTC day and save a chart.` In Claude Code, invoke `/logcove`.

The Skill guides discovery, manifest-based local DuckDB analysis, Vega-Lite generation, and optional Chart persistence. It does not ask the model to manage Session tokens or use Vector write keys for reads.

## License

Apache-2.0.
