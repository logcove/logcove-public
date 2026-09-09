# Homebrew and WinGet distribution

## Status

Homebrew and WinGet are the selected package-manager channels. There is no npm package. The repository remains private until the product launch; package metadata generation does not change its visibility, create a public tap, or submit a public WinGet manifest.

Version 0.2.0 includes the generator and CI validation. Install the CLI from its release archive. Public package-manager installation is not available yet: both channels require release URLs that ordinary users can download without GitHub authentication.

## User installation after launch

For macOS and Linux with Homebrew:

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

CLI 0.2.0 also provides the matching Skill without a separate download:

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

The metadata ZIP is included in the next Release and SHA256SUMS. Generating it does not publish to Homebrew or WinGet. Ruby/schema checks do not establish a real package-manager install or WinGet review acceptance.

## Local validation (2026-09-09)

- macOS: formatting and strict Clippy passed; the full Rust suite passed 61 tests, with one real credential-store test intentionally ignored. The 0.2.0 ARM64 release archive was built, extracted and smoke-tested.
- Linux ARM64: seven Skill installer tests passed in a disposable Debian container. The actual CLI installed for Codex and Claude with a deliberately invalid API URL and corrupt configuration; a repeated Codex install returned `unchanged`. This did not modify the host's installed Skills or test Linux login.
- Seven Python packaging tests, Skill/DuckDB reference checks and release-shell simulations passed. GitHub workflow syntax passed actionlint 1.7.12.
- Metadata generated from the real 0.1.0 release archives passed official WinGet 1.9.0 schema checks, SHA-256 comparisons and the nested executable-path check. The Homebrew formula passed Ruby syntax validation and loaded in the installed macOS Homebrew with the correct ARM64 URL/hash. A separate local copy of those six release archives plus generated metadata passed the new seven-archive checksum flow; the published 0.1.0 release was not modified.

These are local checks. The updated 0.2.0 five-platform GitHub workflow has not run yet. Actual Homebrew/WinGet installation, native WinGet validation, anonymous downloads and public package-index acceptance remain unverified.

## Publication at product launch

1. Make the product repository public as explicitly authorized at launch, and verify anonymous access to the release files. This is currently deferred.
2. Create `logcove/homebrew-tap`, copy the generated `Formula/logcove.rb`, and test installation. A tap is a GitHub repository; no separate Homebrew upload account is required. Initial updates can use the maintainer's GitHub access. Cross-repository CI updates would require a separately scoped GitHub credential and are not configured here.
3. Validate and test the WinGet manifest on Windows, then submit the generated manifest directory to `microsoft/winget-pkgs`. Availability through `winget install` depends on acceptance into that index, not just a GitHub Release.
4. Promote the package-manager commands to the primary README installation path once they work for ordinary users. Keep binary downloads as an alternative.

References: [Homebrew taps](https://docs.brew.sh/Taps), [Homebrew formula cookbook](https://docs.brew.sh/Formula-Cookbook), [WinGet manifest documentation](https://learn.microsoft.com/windows/package-manager/package/manifest), and [WinGet schemas](https://github.com/microsoft/winget-cli/tree/master/schemas/JSON/manifests/v1.9.0).
