# Logcove CLI and Skills

Use your own computer to analyze Logcove logs and create charts with your coding agent.

This public repository contains the Rust CLI, agent Skills, and user documentation. The CLI talks only to Logcove's public user APIs. Log collection, the web application, and service deployment are maintained separately.

## Development status

The implementation plan is in [docs/implementation-plan.md](docs/implementation-plan.md). The CLI implements configuration, browser authorization, persistent sessions, Project discovery, Parquet downloads, and Chart operations. See [validation results](docs/validation.md) for actual platform and API checks. The analysis Skill and binary distribution follow in batch 3.

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

## License

Apache-2.0.
