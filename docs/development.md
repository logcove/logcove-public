# Development guide

For installation and a user walkthrough, start with the [README](../README.md).

## Repository scope

This repository contains the Rust CLI, the shared Codex/Claude Code Skill, and user documentation. The CLI talks only to Logcove's public user APIs. Log collection, the web application, and service deployment are maintained separately; this checkout does not start a Logcove backend.

The CLI handles configuration, browser authorization, persistent sessions, Project discovery, concurrent Parquet downloads, and Chart operations. It does not embed DuckDB or call an LLM. Agents use an independently installed DuckDB for computation and the CLI for service operations. The Skill guides source discovery, manifest-based analysis, Vega-Lite generation and optional Chart persistence without asking agents to manage Session tokens or use Vector write keys for reads.

| Path | Contents |
| --- | --- |
| `cli/` | Rust CLI and contract tests |
| `skills/logcove/` | Shared Skill entrypoint and reference instructions |
| `scripts/` | Packaging and CI validation scripts |
| `.github/workflows/` | CI, platform builds and release workflow |
| `docs/` | User guides, design plans and validation records |

Read [AGENTS.md](../AGENTS.md) before changing code. Keep credentials, real logs, local experiments and private deployment details out of this repository.

## Local development

Use Rust 1.90 or newer and the committed Cargo.lock. End users can install the prebuilt CLI from the [Release](https://github.com/logcove/logcove-public/releases/tag/v0.2.0); building from source is for development.

```sh
git clone https://github.com/logcove/logcove-public.git
cd logcove-public
```

```sh
cargo build --locked
cargo run --locked -- --help
```

To install your source build into Cargo's executable directory, run `cargo install --path cli --locked`. Ensure `~/.cargo/bin` (or `%USERPROFILE%\.cargo\bin` on Windows) is on PATH.

With a separate local API running at `http://localhost:8787` and its associated web application available, configure and try the CLI:

```sh
cargo run --locked -- config set-api-url http://localhost:8787
cargo run --locked -- login
cargo run --locked -- whoami
cargo run --locked -- projects list
```

These commands use the normal CLI configuration and credential store. See [API environment selection](cli.md#select-an-api-environment) for configuration precedence and isolation options. Replace localhost with an actual deployment origin when appropriate.

## Checks

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --release --locked
```

Normal Rust tests use HTTP stubs and fake credential storage. The real OS credential test is explicitly ignored by default; its prerequisites and invocation are in [CLI validation](validation.md).

Skill and packaging checks use Python 3.12 or newer. Packaging uses the standard library; Skill and workflow checks use the pinned CI dependencies in `scripts/requirements-ci.txt`. For environment setup and commands, see [local release verification](releases.md#local-verification).

## Implementation and release status

Version 0.2.0 adds local `skills install` commands and generation of Homebrew/WinGet metadata from built archives. The repository remains private and no package-index submission is configured; see [package-manager distribution](package-managers.md).

The CLI and shared analysis Skill are implemented. Five-platform CI, archive packaging and tag-triggered release automation are also implemented, with successful hosted checks for macOS ARM64/x64, Linux ARM64/x64 and Windows x64. A passing build does not establish real browser login, native credential-store behavior or installation on every supported desktop.

The first binary release is `v0.1.0`, with five platform archives, a standalone Skill ZIP and SHA256SUMS. Homebrew/WinGet publication and fresh-environment installation tests remain pending; the README documents manual installation of released binaries. Do not push a version tag to test ordinary CI: tag pushes trigger publication after the checks pass.

- [Implementation plan](implementation-plan.md): scope, responsibilities and delivery batches.
- [CLI validation](validation.md): automated checks, live workflows and platform boundaries.
- [Skill validation](skill-validation.md): reference examples and agent-workflow verification boundaries.
- [Release guide](releases.md): platform matrix, packaging, publishing procedure and hosted CI evidence.
