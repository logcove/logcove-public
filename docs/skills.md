# Logcove analysis Skill

The instruction-only Skill is in [`skills/logcove`](../skills/logcove/SKILL.md). It uses the Rust CLI for Logcove operations, an independently installed DuckDB for local computation, and Vega-Lite for chart specifications. The same folder works with Codex and Claude Code. No MCP configuration, API key prompt, bundled query engine, or extra CLI command is required.

## Prerequisites

- Install the [prebuilt CLI](../README.md#install), and ensure `logcove` is on the agent's PATH. Its archive includes the matching `skills/logcove/` folder; the Skill is also available as a [standalone ZIP](https://github.com/logcove/logcove-public/releases/download/v0.1.0/logcove-skills-v0.1.0.zip).
- Configure your actual API origin and complete `logcove login`. See [CLI usage](cli.md) for browser authorization and OS credential-store requirements.
- Provide a local DuckDB environment. Either the DuckDB CLI or Python package can be used; the Skill's examples use Python's `duckdb` package. A renderer is optional for local preview but necessary before claiming visual verification or image export.

The Skill can guide login and source selection, but it does not supply a hosted Logcove deployment or create an account. A future container can reuse the instructions with suitable dependencies and its own authorized identity; noninteractive container authentication is not implemented here.

## Install from a CLI package or checkout

Copy the **whole `skills/logcove` directory**, including `references`, into the host's skills directory. Installing only `SKILL.md` loses the command and data-format references.

| Host | User-wide destination | Project-only destination | Explicit invocation |
| --- | --- | --- | --- |
| Codex | `~/.agents/skills/logcove/` | `.agents/skills/logcove/` | `$logcove` |
| Claude Code | `~/.claude/skills/logcove/` | `.claude/skills/logcove/` | `/logcove` |

From the root of the extracted CLI package (or source checkout), for a first user-wide install on macOS/Linux:

```sh
# Codex
mkdir -p "$HOME/.agents/skills"
cp -R skills/logcove "$HOME/.agents/skills/"

# Claude Code
mkdir -p "$HOME/.claude/skills"
cp -R skills/logcove "$HOME/.claude/skills/"
```

Run only the host-specific pair you need, or both if you use both tools. To use Windows PowerShell, the equivalent first install is:

```powershell
# Codex; use .claude instead of .agents for Claude Code
New-Item -ItemType Directory -Force "$env:USERPROFILE\.agents\skills" | Out-Null
Copy-Item -Path skills/logcove -Destination "$env:USERPROFILE\.agents\skills" -Recurse
```

For development, Codex also supports symlinked Skill folders. Otherwise refresh the installed copy after changing the source, preserving any deliberate local customizations. Verify which copy the host loads if you have installed the same Skill in more than one scope. Start a fresh session if it is not visible.

These are local installation instructions, not a plugin-marketplace release or automated updater. Host discovery paths were checked against the [official Codex documentation](https://developers.openai.com/codex/skills/) and [Claude Code documentation](https://code.claude.com/docs/en/skills) on 2026-09-08. Actual verification boundaries are in [Skill validation](skill-validation.md).

## Distribution archives

The build workflow packages this same Skill with the CLI's version. A CLI archive includes `skills/logcove/`, so the checkout copy instructions above also work from its extracted root. The separate `logcove-skills-v<version>.zip` extracts directly to `logcove/`; copy that entire folder into the appropriate host directory from the table above. Do not copy only SKILL.md. The separate ZIP includes a VERSION file and LICENSE.

Check the release's SHA256SUMS before extracting. Release publication and automated installation/update are distinct: the tag-triggered release workflow is implemented, but installation/update scripts and fresh-environment verification remain the next batch. See [release procedures](releases.md) for the actual publication status.

## Try a task

For Codex:

```text
Use $logcove to list my readable sources, then analyze requests by service
for the UTC ingestion day 2026-09-08. Save the chart after computing it.
```

For Claude Code:

```text
/logcove Analyze errors for my API source over the last complete UTC day.
Keep the results local and generate a Vega-Lite chart without saving it.
```

Replace the example day/source with actual available data. A task may span multiple Projects. The agent should clarify an ambiguous source or period when necessary, inspect the schema, use completed download manifests, calculate in DuckDB, and explain the coverage. Saving uploads the selected aggregate rows, SQL, spec, and metadata to Logcove; a local-only task should not create or overwrite a saved Chart.

The Skill supports creating Charts and updating existing ones, including replacing the latest result. It does not add scheduled refresh, alerts, a dashboard, or image-export commands to the CLI.

## Repository structure

```text
skills/logcove/
  SKILL.md
  references/
    cli.md
    duckdb.md
    charts.md
```

The entrypoint routes to references as needed. References stay inside the Skill folder so a copied installation does not depend on the rest of this repository or any private backend repository. Keep references aligned with CLI contracts when commands or result formats change.
