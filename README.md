# Logcove CLI and Skills

Analyze your Logcove logs and create charts with Codex or Claude Code, using your own computer for DuckDB calculations.

Download Parquet from your data sources, explore logs across Projects, and generate Vega-Lite charts. Keep an analysis local or save its chart definition to calculate and view in Logcove.

## Install

Download the **v0.2.0** package for your computer. No Rust toolchain or source checkout is needed.

| System | Download |
| --- | --- |
| macOS, Apple Silicon (M-series) | [macOS ARM64](https://github.com/logcove/logcove-public/releases/download/v0.2.0/logcove-v0.2.0-aarch64-apple-darwin.tar.gz) |
| macOS, Intel | [macOS x64](https://github.com/logcove/logcove-public/releases/download/v0.2.0/logcove-v0.2.0-x86_64-apple-darwin.tar.gz) |
| Linux, x64 | [Linux x64](https://github.com/logcove/logcove-public/releases/download/v0.2.0/logcove-v0.2.0-x86_64-unknown-linux-gnu.tar.gz) |
| Linux, ARM64 | [Linux ARM64](https://github.com/logcove/logcove-public/releases/download/v0.2.0/logcove-v0.2.0-aarch64-unknown-linux-gnu.tar.gz) |
| Windows, x64 | [Windows x64](https://github.com/logcove/logcove-public/releases/download/v0.2.0/logcove-v0.2.0-x86_64-pc-windows-msvc.zip) |

Extract the archive and open a terminal in the extracted `logcove-v0.2.0-...` folder. It contains the CLI, the Skill, and usage guides. [Release notes](https://github.com/logcove/logcove-public/releases/tag/v0.2.0) and [SHA-256 checksums](https://github.com/logcove/logcove-public/releases/download/v0.2.0/SHA256SUMS) are available with the download.

**macOS / Linux**

```sh
mkdir -p "$HOME/.local/bin"
install -m 755 ./logcove "$HOME/.local/bin/logcove"
export PATH="$HOME/.local/bin:$PATH"
logcove --version
```

Add the `export PATH=...` line to your shell profile if `~/.local/bin` is not already on PATH.

**Windows PowerShell**

```powershell
New-Item -ItemType Directory -Force "$env:LOCALAPPDATA\Logcove\bin" | Out-Null
Copy-Item .\logcove.exe "$env:LOCALAPPDATA\Logcove\bin\logcove.exe" -Force
$env:Path = "$env:LOCALAPPDATA\Logcove\bin;$env:Path"
logcove --version
```

Add `%LOCALAPPDATA%\Logcove\bin` to your user Path in Windows Environment Variables to use it in future terminals.

Linux login requires a running, unlocked Secret Service; see [credential storage](docs/cli.md#credential-persistence). Packages are currently unsigned; see the [platform notes](docs/releases.md#version-and-targets) for compatibility details.

For analysis, you also need Codex or Claude Code and a local [DuckDB](https://duckdb.org/docs/installation/) CLI or Python environment. DuckDB is installed separately from Logcove.

## Quick start

### 1. Connect and sign in

You need a running Logcove deployment and an account. Replace the example address with the API origin supplied by your deployment:

```sh
logcove config set-api-url https://your-api.example.com
logcove login
logcove whoami
logcove projects list
```

`login` opens your browser. Sign in and approve the CLI request; your session is saved in your OS credential store. For a link you can open manually, use `logcove login --no-browser`.

The CLI currently requires an explicit API address. Each Project in the list is a data source you can explore.

### 2. Install the Skill

Run the command for your agent on macOS, Linux or Windows. The Skill is bundled in the CLI; installation does not require a separate download or login.

**Codex**

```sh
logcove skills install --agent codex
```

**Claude Code**

```sh
logcove skills install --agent claude
```

Start a new agent session if the Skill is not visible. If an older or customized copy is installed, review your changes and rerun with `--force` to replace bundled files. Extra user files are preserved. A [standalone Skill ZIP](https://github.com/logcove/logcove-public/releases/download/v0.2.0/logcove-skills-v0.2.0.zip) and [manual installation instructions](docs/skills.md#install-from-a-cli-package-or-checkout) are also available.

### 3. Ask about your logs

In Codex:

```text
$logcove List my data sources, then help me analyze errors
for the last complete UTC day. Keep the analysis local for now.
```

In Claude Code:

```text
/logcove List my data sources, then help me analyze errors
for the last complete UTC day. Keep the analysis local for now.
```

The agent uses the CLI to download your data and DuckDB to calculate locally. When you want to save a chart, ask it to:

```text
Create a chart from this analysis and save it to Logcove.
```

Saving a chart uploads its SQL, Vega-Lite specification and metadata. Results stay local; the web or desktop application calculates the selected time range on your device. Definition-only Charts require the upcoming CLI/API version; published v0.2.0 still uses the previous result contract. You can also use the CLI directly; see the [download and Chart walkthrough](docs/cli.md#download-parquet).

## Guides

- [CLI reference](docs/cli.md): configuration, login, downloads and chart commands.
- [Skill guide](docs/skills.md): installation, analysis workflow and usage examples.
- [Homebrew and WinGet](docs/package-managers.md): availability and installation at product launch.
- [Development guide](docs/development.md): source layout, checks and release procedures.

## License

[Apache-2.0](LICENSE).
