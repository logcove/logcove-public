# Homebrew and WinGet distribution

## Status

Homebrew and WinGet are the selected package-manager channels. There is no npm package. The repository is public as of 2026-09-23. All seven v0.3.0 archives were downloaded anonymously and verified against the release's SHA256SUMS.

Install the CLI from its [release archive](../README.md#install-manually), or give the [installation guide](../INSTALL.md) to your agent. Public package-manager installation is not available yet: the Homebrew tap has not been published, and no WinGet manifest has been submitted.

## Package-manager commands after publication

After `logcove/homebrew-tap` is published, macOS and Linux users with compatible runtimes can use:

```sh
brew install logcove/tap/logcove
brew upgrade logcove/tap/logcove
```

For Windows, after the manifest is accepted into the WinGet community repository:

```powershell
winget install --id Logcove.Logcove --exact --source winget
winget upgrade --id Logcove.Logcove --exact --source winget
```

`Logcove.Logcove` is the proposed package identifier, not an already registered package. Both installation definitions download a prebuilt binary rather than compile Rust. Linux uses the existing GNU/glibc archives and still needs a running, unlocked Secret Service for login.

CLI 0.2.0 and newer also provide the matching Skill without a separate download:

```sh
logcove skills install --agent codex
logcove skills install --agent claude
```

After a CLI upgrade, review any local Skill changes, then use `--force` to replace files from an older version. See [Skill installation](skills.md#install-with-the-cli-020) for the exact behavior. This command is not included in the published 0.1.0 binary.

## Generated metadata

After the five platform archives have been built, run:

```sh
python3 scripts/package_managers.py --assets dist
```

The generator reads the current Cargo version and calculates SHA-256 directly from each archive. It writes `dist/logcove-package-managers-v<version>.zip`, containing:

```text
package-managers/
  Formula/logcove.rb
  manifests/l/Logcove/Logcove/<version>/
    Logcove.Logcove.yaml
    Logcove.Logcove.locale.en-US.yaml
    Logcove.Logcove.installer.yaml
  README.md
```

The formula selects macOS/Linux and ARM64/x64 using Homebrew's platform declarations. Its install step places the binary on PATH; it does not run Cargo. The WinGet installer manifest uses ZIP + portable with an explicit `logcove.exe` path inside the Windows archive and the `logcove` command alias.

For maintainer validation against an already downloaded release, `--version 0.1.0` selects that version's archive names. This generates local metadata only and does not alter that published release.

The shared CI workflow waits for all CLI and Skill packages, generates the metadata, checks Homebrew URL/hash pairs and Ruby syntax, and validates the three WinGet YAML files against Microsoft's versioned 1.9.0 JSON schemas. It also checks that WinGet's nested executable path exists in the real ZIP. `jsonschema` is a CI-only dependency; the generator and CLI add no runtime dependency. Schema validation fetches the official versioned schema files and requires network access, but no user credentials.

The metadata ZIP is included in v0.3.0 and SHA256SUMS. Generating it does not publish to Homebrew or WinGet. Ruby/schema checks do not establish a real package-manager install or WinGet review acceptance.

## Public-release preparation (2026-09-23)

- Verified anonymous downloads and SHA-256 for all five CLI archives, the Skill ZIP and package-manager ZIP.
- Validated the released formula's four URLs/hashes, Ruby syntax, all three WinGet manifests against Microsoft's schemas, and the executable path inside the actual Windows ZIP.
- Prepared a Homebrew tap with the unchanged released formula, README and Apache-2.0 license in `dist/public-launch-20260923/homebrew-tap/`.
- On macOS ARM64, installed that formula through a temporary local tap using the public release URL; `brew test`, version output, PAT help and Skill-install help passed. The temporary installation was removed afterward. This verifies this machine, not a fresh macOS installation or the not-yet-published remote tap.
- Extracted the matching WinGet manifests into `dist/public-launch-20260923/metadata/package-managers/manifests/`.
- Prepared an [agent installation guide](../INSTALL.md). Its public raw URL becomes available after this document change is pushed; it does not require a CLI rebuild or a website deployment.

These local staging directories are ignored by Git. Release assets remain unchanged. The historical validation notes below are not claims about current public channel availability.

### Runtime requirements

The existing Windows manifest does not declare the x64 Visual C++ Runtime dependency. Before submitting it to WinGet, add the dependency to the generator, validate with native WinGet, and test installation on Windows without a preinstalled runtime. This remains deferred; schema validation alone does not cover it.

Linux ARM64 binaries use Ubuntu 24.04 / glibc 2.39. The current formula does not reject older glibc environments in advance. Document this requirement and verify Linux installation before claiming general Linux compatibility. Browser login also needs an unlocked Secret Service; PAT authentication does not.

## Local validation (2026-09-09)

- macOS: formatting and strict Clippy passed; the full Rust suite passed 61 tests, with one real credential-store test intentionally ignored. The 0.2.0 ARM64 release archive was built, extracted and smoke-tested.
- Linux ARM64: seven Skill installer tests passed in a disposable Debian container. The actual CLI installed for Codex and Claude with a deliberately invalid API URL and corrupt configuration; a repeated Codex install returned `unchanged`. This did not modify the host's installed Skills or test Linux login.
- Seven Python packaging tests, Skill/DuckDB reference checks and release-shell simulations passed. GitHub workflow syntax passed actionlint 1.7.12.
- Metadata generated from the real 0.1.0 release archives passed official WinGet 1.9.0 schema checks, SHA-256 comparisons and the nested executable-path check. The Homebrew formula passed Ruby syntax validation and loaded in the installed macOS Homebrew with the correct ARM64 URL/hash. A separate local copy of those six release archives plus generated metadata passed the new seven-archive checksum flow; the published 0.1.0 release was not modified.

At that date these were local checks, before the updated 0.2.0 five-platform workflow ran. Refer to the current public-release preparation record above for later verification; native WinGet installation and package-index acceptance remain unverified.

## Remaining publication steps

1. Commit and push the public-state documentation and INSTALL.md; check the raw installation-guide URL without authentication.
2. Publish the prepared `logcove/homebrew-tap` repository. A tap is a GitHub repository; no separate Homebrew upload account is required. Initial updates can use the maintainer's GitHub access. Cross-repository CI updates would require a separately scoped GitHub credential and are not configured here.
3. Verify `brew install logcove/tap/logcove` through the actual public tap, then run `brew test logcove/tap/logcove`. Confirm version and PAT support from the installed executable. Promote Homebrew in the main README only after this succeeds.
4. Resolve the Windows runtime dependency, validate and test the WinGet manifest on Windows, then submit it to `microsoft/winget-pkgs`. Promote the WinGet command only after acceptance into that index.

Do not overwrite v0.3.0 assets or move its tag to publish these documents or the tap. Subsequent CLI releases must use a new version; copy their matching generated formula into the tap. Manual tap updates are the initial workflow. Installation-guide updates also need to track the new release and its platform requirements.

References: [Homebrew taps](https://docs.brew.sh/Taps), [Homebrew formula cookbook](https://docs.brew.sh/Formula-Cookbook), [WinGet manifest documentation](https://learn.microsoft.com/windows/package-manager/package/manifest), and [WinGet schemas](https://github.com/microsoft/winget-cli/tree/master/schemas/JSON/manifests/v1.9.0).
