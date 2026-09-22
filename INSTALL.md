# Install Logcove with your agent

Use this guide when the user asks you to install the Logcove CLI and its Skill.
The public repository is https://github.com/logcove/logcove-public. The current
stable release is **v0.3.0**, including personal-token support.

Install prebuilt binaries; a Rust toolchain or source checkout is not required.
Homebrew and WinGet publication is still pending. Use the release archives below
until those channels are marked available in the repository's package-manager guide.

## 1. Inspect the environment

- Determine the operating system, CPU architecture, shell, and current agent
  (Codex or Claude Code). Ask only if the target agent is not clear.
- Check whether `logcove` is already on PATH. If present, run `logcove --version`
  and `logcove --help`. The rebuilt v0.3.0 mentions `LOGCOVE_TOKEN` in root help;
  the original v0.3.0 does not. A version number alone cannot distinguish them.
- Reuse a compatible current installation. Do not downgrade a newer version,
  overwrite a package-manager installation with a manual copy, reset API
  configuration, or delete saved credentials.
- Check only the presence of `LOGCOVE_TOKEN`, never print its value. Installation
  itself needs no Logcove login, PAT, GitHub token, or storage credentials.

## 2. Download and verify a binary

Download the matching asset and `SHA256SUMS` from this exact release directory:

https://github.com/logcove/logcove-public/releases/download/v0.3.0/

| System | Asset |
| --- | --- |
| macOS Apple Silicon | `logcove-v0.3.0-aarch64-apple-darwin.tar.gz` |
| macOS Intel | `logcove-v0.3.0-x86_64-apple-darwin.tar.gz` |
| Linux x64 | `logcove-v0.3.0-x86_64-unknown-linux-gnu.tar.gz` |
| Linux ARM64 | `logcove-v0.3.0-aarch64-unknown-linux-gnu.tar.gz` |
| Windows x64 | `logcove-v0.3.0-x86_64-pc-windows-msvc.zip` |

These are direct public downloads. Use an HTTP client that follows GitHub's
release-asset redirects and reports HTTP errors. Download to a temporary directory.
Compare the archive's SHA-256 with the matching filename in `SHA256SUMS` **before
extracting or running it**. Use `shasum -a 256`, `sha256sum`, or PowerShell's
`Get-FileHash -Algorithm SHA256`. Stop if the entry is missing or the digest differs.

The archives extract into a `logcove-v0.3.0-<target>` directory. They contain the
executable, license, documentation, and Skill sources.

Compatibility boundaries:

- Linux archives require GNU/glibc, not Alpine/musl. The supported build baselines
  are Ubuntu 22.04 for x64 and Ubuntu 24.04 / glibc 2.39 for ARM64. Do not claim
  older Linux distributions are supported without testing them.
- Windows uses the x64 Microsoft Visual C++ Runtime. If it is missing, use the
  official Microsoft redistributable, not a third-party DLL download.
- Packages are unsigned and macOS binaries are not notarized. Report any OS
  trust prompt; do not disable platform security protections automatically.

## 3. Install for this user

For a manual macOS/Linux installation, put the executable at
`~/.local/bin/logcove` with executable permissions. For Windows, use
`%LOCALAPPDATA%\Logcove\bin\logcove.exe`. No system-wide installation is needed.

Ensure that directory is on the user's PATH and available to the agent. Preserve
existing shell/profile entries and avoid adding duplicate PATH entries. In the
current session, an absolute executable path may be needed until PATH is refreshed.
Check which executable `logcove` resolves to so an older copy does not shadow it.

Run the installed binary's `--version` and `--help`. Confirm both version 0.3.0
(or a newer compatible version) and `LOGCOVE_TOKEN` support.

## 4. Install the matching Skill

Run only the command for the current agent, unless both were requested:

```sh
logcove skills install --agent codex
logcove skills install --agent claude
```

The CLI installs its bundled Skill and all references without a network request.
Codex uses `~/.agents/skills/logcove`; Claude Code uses `~/.claude/skills/logcove`.
An identical installation is unchanged. If an existing copy conflicts, inspect
whether it contains user customizations before using `--force`; preserve those
customizations. Do not copy only SKILL.md or overwrite a symlinked development copy.

Check the command's successful JSON result and returned directory. Start a fresh
agent session if the host has not discovered the new Skill yet.

## 5. Verify authentication when the user is ready

The default API is `https://api.logcove.com`. Ordinary users do not need to set an
API URL. Preserve an existing explicit override; do not silently change environments.

Run `logcove whoami`. If `LOGCOVE_TOKEN` is set, the CLI uses it ahead of any saved
login, including when it is empty or invalid. If rejected, ask the user to correct
their secret-store configuration; do not print it, unset it, or fall back to login.

If no PAT is set and the CLI reports `UNAUTHENTICATED`, use `logcove login` and let
the user approve in their own browser. `logcove login --no-browser` supplies a
manual link. Keep that process running while waiting. Never ask the user to paste
passwords or tokens into chat. Linux browser login needs a running, unlocked
Secret Service; PAT mode bypasses the OS credential store.

After successful authentication, `logcove whoami` and `logcove projects list`
verify the account and access. An empty Project list is a valid result. Installation
does not authorize creating Projects, keys, charts, or sending test logs.

## 6. Report what is ready

Report the installed CLI version and path, the Skill's host and installation
directory, and whether authentication was verified or is still waiting for the
user. Do not describe a pending browser approval as a completed login.

DuckDB is separate from the CLI. Check for an existing DuckDB CLI or Python
environment before analysis. If it is absent, explain that local computation
still needs it; use the user's chosen environment instead of modifying global
Python packages. There is no `logcove query` command.

A first analysis prompt after setup:

```text
Use the Logcove Skill to list my data sources and analyze errors for the last
complete UTC day. Keep the analysis local for now.
```
