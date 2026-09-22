# CLI and Skill releases

## Status and scope

### Public distribution (2026-09-23)

The repository is public. All seven v0.3.0 archives and SHA256SUMS were downloaded
without GitHub authentication; every archive matched its published checksum.
The September 19 replacement is the current released v0.3.0, including PAT support.
Older entries below record the preparation state on their respective dates.

The [agent installation guide](../INSTALL.md) supports installing a release binary
and its bundled Skill. It is included in future CLI archives; existing release
assets remain unchanged. Homebrew tap and WinGet publication have their own
[status and remaining steps](package-managers.md).

### v0.3.0 replacement (2026-09-19)

The maintainer explicitly requested replacing v0.3.0 with the personal-token CLI
and updated Skill. This is a one-time exception to the immutable-release policy
below; the workflow's published-release protection remains unchanged. The rebuild
keeps version 0.3.0, so users must replace the binary and check `--help` for
`LOGCOVE_TOKEN`, then reinstall the bundled Skill. Original v0.3.0 binaries ignore
that environment variable. See the [updated release notes](release-notes/v0.3.0.md).

The replacement procedure backs up the old assets and tag, converts the Release
to a draft, updates the tag to the reviewed commit, and runs all five targets and
Skill checks before publishing replacement assets and fresh package-manager hashes.
Completion must be verified from the new hosted run and downloaded Release assets.
This CLI release does not deploy the API/app; PAT use requires that separate
deployment. Repository visibility and public package-manager publication are unchanged.

### v0.3.0 preparation (2026-09-17)

Version 0.3.0 is prepared in the working tree; no v0.3.0 tag or Release has been published by this preparation. The README and user guides target this version. [Release notes and upgrade instructions](release-notes/v0.3.0.md) cover the default production API, Project/Key management, local download reuse, definition-only Charts, and the updated Skill. The publishing workflow reads this versioned notes file, including when retrying an existing draft.

Homebrew/WinGet publication and repository visibility changes remain separate actions. Preparing a release does not enable public package-manager installation.

Local source checks passed: 78 Rust tests (one real OS credential test remains explicitly ignored), strict Clippy and formatting, seven packaging tests, two Skill checks and two publishing-workflow tests. A native macOS ARM64 release build passed. The other four platform builds and the hosted v0.3.0 workflow remain to be verified at the release commit.

The [v0.2.0 release](https://github.com/logcove/logcove-public/releases/tag/v0.2.0) packages five prebuilt CLI archives, the matching Skill ZIP, Homebrew/WinGet metadata and SHA256SUMS. Users install downloaded binaries; source builds are documented in the [development guide](development.md). All five platform jobs and the Skill/release checks passed on GitHub Actions for commit `90ccde0` on 2026-09-09; see the hosted validation record below. The release workflow also runs the full checks at its own tagged commit. Hosted runner checks do not replace fresh-machine installation or real browser/credential-store acceptance.

The original batch 2 installation/update-script plan is superseded by Homebrew and WinGet distribution plus a bundled Skill installer. Fresh-environment package-manager installation remains pending. There is no self-update command, automatic Skill updater, bundled DuckDB, or code signing/notarization.

Version 0.2.0 introduced the bundled `skills install` command and Homebrew/WinGet metadata generation. Those channels were deferred until product launch; see the current [package-manager distribution](package-managers.md) status. There is no npm distribution.

Version 0.3.0 defaults to `https://api.logcove.com` and preserves flag, environment and saved-origin overrides. Published v0.1.0 and v0.2.0 require explicit configuration. Publishing a CLI release does not deploy the API.

## Version and targets

CLI and Skill use the version in `cli/Cargo.toml`. A release tag must be exactly `v` plus that version, for example `v0.1.0`. Update Cargo.lock alongside Cargo.toml when changing the package version. A prerelease such as `v0.2.0-rc.1` must match Cargo's prerelease version and is marked as a GitHub prerelease. Skill-only releases also increment the shared package version.

| Platform | Rust target | GitHub runner | Archive |
| --- | --- | --- | --- |
| macOS Apple Silicon | `aarch64-apple-darwin` | `macos-15` | `.tar.gz` |
| macOS Intel | `x86_64-apple-darwin` | `macos-15-intel` | `.tar.gz` |
| Linux x64 | `x86_64-unknown-linux-gnu` | `ubuntu-22.04` | `.tar.gz` |
| Linux ARM64 | `aarch64-unknown-linux-gnu` | `ubuntu-24.04-arm` | `.tar.gz` |
| Windows x64 | `x86_64-pc-windows-msvc` | `windows-2022` | `.zip` |

Each job runs on its target architecture and uses Rust 1.90.0 with `--locked`. The Linux packages target GNU/glibc environments, not Alpine/musl. Ubuntu 22.04 x64 and Ubuntu 24.04 ARM64 are the initial build/test baselines; compatibility with older systems needs separate verification. Linux login additionally requires a running, unlocked Secret Service. macOS/Windows runner checks do not establish support for every desktop OS version or real browser/credential-store behavior. The native credential test remains explicitly ignored unless run on a configured desktop.

Runner labels are from [GitHub-hosted runner documentation](https://docs.github.com/en/actions/reference/runners/github-hosted-runners). The repository must have Actions enabled and access to these runners, including ARM64 Linux.

### Known installation limitations carried forward from 0.2.0

- The Windows MSVC executable depends on the x64 Visual C++ Runtime (`VCRUNTIME140.dll`). The prepared WinGet manifest does not yet declare that dependency; users need the runtime installed separately.
- The Linux ARM64 release uses the Ubuntu 24.04/glibc 2.39 baseline. The 0.1.0 archive from this unchanged build configuration fails to start on Debian 12 with missing `GLIBC_2.38`/`GLIBC_2.39` errors. The Homebrew formula does not yet check that requirement, and installation on older glibc systems is unverified.

These two compatibility fixes remain deferred. They must be accounted for when publishing package-manager support; repository visibility alone does not resolve them.

## Checks and packaging

`.github/workflows/ci.yml` runs on branch pushes, pull requests, and manual dispatch. It calls `checks.yml`, which is also reused by `release.yml` at the tagged commit:

- All five targets: rustfmt, Clippy with warnings denied, Rust unit/contract tests, release build, and Python packaging tests.
- Each CLI archive is extracted and the extracted executable runs version, command help, and isolated configuration checks. Shell examples in Skill references are checked through Clap's help handling; no example API operations run.
- The Skill job validates YAML metadata and internal references, then executes the actual DuckDB reference snippets against temporary synthetic Parquet files. Checks cover exact manifest files, schema evolution, aggregates, result fields, empty results versus no files, and row/size/alias/NaN rejection.
- Packaging tests cover tag mismatch, archive contents and extraction, executable permissions for tar archives, and rejecting incomplete or unexpected release asset sets.
- A distribution job waits for all platform/Skill artifacts, generates Homebrew and WinGet metadata from their hashes, checks formula syntax and official WinGet schemas, and generates the complete checksum file. It uploads a `package-managers` artifact for publication; it does not push a tap or submit to WinGet.
- The publishing shell block is tested with a fake `gh` command: first publish, draft retry, refusing a public release overwrite, prerelease marking, and upload failure preventing publication. This is simulation, not a real GitHub release test.

CI uses Python 3.12. Packaging and metadata generation use only its standard library; checks additionally install pinned DuckDB, PyYAML and jsonschema from `scripts/requirements-ci.txt`. These are CI dependencies, not CLI runtime dependencies. WinGet schema validation fetches official versioned schemas. No test account, R2 credentials, OAuth secrets, or LLM access is required. These checks do not render Vega charts or validate an independent agent conversation.

Branch and PR runs upload the same package format as Actions artifacts, retained for seven days. They are development artifacts, not published releases. `dist/` is ignored by Git. Packaging copies an explicit file list rather than arbitrary repository contents or local experiments.

## Release assets

Starting with version `0.2.0`, a complete release has seven archives plus one checksum file (the released `0.1.0` has no package-manager metadata archive):

```text
logcove-v0.3.0-aarch64-apple-darwin.tar.gz
logcove-v0.3.0-x86_64-apple-darwin.tar.gz
logcove-v0.3.0-x86_64-unknown-linux-gnu.tar.gz
logcove-v0.3.0-aarch64-unknown-linux-gnu.tar.gz
logcove-v0.3.0-x86_64-pc-windows-msvc.zip
logcove-skills-v0.3.0.zip
logcove-package-managers-v0.3.0.zip
SHA256SUMS
```

Each CLI archive has a `logcove-v<version>-<target>/` root containing its executable, LICENSE, README.txt, the user-facing README.md, matching public documentation and the small `skills/logcove/` source folder. AGENTS.md is included for the development guide's reference; future archives also include INSTALL.md. This keeps documentation and Skill installation instructions usable after extraction. The separate Skill ZIP supports users who only need the Skill; it contains a directly installable `logcove/` folder with SKILL.md, all four references, LICENSE and VERSION. Install the complete folder using [Skill installation](skills.md).

Check downloaded archives against SHA256SUMS before use (`sha256sum` on Linux, `shasum -a 256` on macOS, or `Get-FileHash -Algorithm SHA256` on Windows). These hashes detect file mismatch; they are not a substitute for platform code signing. Downloaded unsigned executables may have a different OS launch experience from local builds; fresh-machine verification and signing decisions remain batch 2 work.

## Publishing procedure

1. Update the shared Cargo version and lockfile, review changes and release notes, and commit them. Update README download links to the version being released. State the actual API configuration requirements and supported deployment scope in the release notes.
2. Explicitly authorize and push the matching version tag. Branch pushes and manual CI runs never publish releases.
3. `release.yml` validates the tag before running the shared checks at that commit. Every target and the Skill job must pass.
4. The publish job downloads artifacts from that workflow run, requires exactly the seven expected archives, and writes SHA256SUMS. Only this job gets `contents: write` via GitHub's built-in token; no personal access token is needed. Package-manager metadata remains a Release asset until separately published to the relevant channel.
5. Require a nonempty `docs/release-notes/v<version>.md`, create a draft Release with those notes, upload all assets, then publish it. A failed upload leaves a draft; rerunning the job can replace assets and refresh notes in that draft. An already published Release is never overwritten by this workflow. To correct a published version, publish a new version.

Only jobs within the same run supply release artifacts. Do not move or recreate release tags; fix failures through a workflow rerun on the unchanged tag or a new version. Maintain the versioned release notes with user-visible changes, upgrade instructions and known limitations before tagging.

The brief draft phase is an upload staging step, not another manual approval gate. Pushing the version tag is the publishing action. This implementation alone does not authorize pushing tags or publishing any release.

## Local verification

From the repository root, using Python 3.12 or newer:

```sh
python3 -m unittest discover -s scripts/tests -v
python3 scripts/release.py version --tag v0.2.0
```

Run Skill checks in an isolated environment with `scripts/requirements-ci.txt` installed:

```sh
python scripts/check_skill.py
python scripts/check_release_workflow.py
python scripts/release.py skill
```

Build and package for the local machine's target (Apple Silicon example):

```sh
cargo build --release --locked --target aarch64-apple-darwin
python3 scripts/release.py cli --target aarch64-apple-darwin
```

After collecting all five CLI archives and the matching Skill ZIP, run `python3 scripts/package_managers.py --assets dist`, then `python3 scripts/release.py checksums`. The checksum command intentionally fails until `dist/` contains exactly all seven archives. Do not use placeholder packages for an actual release.

## Batch 1 local validation (2026-09-08)

- macOS ARM64 with Rust 1.90.0: rustfmt, Clippy, all 54 Rust tests and target-specific release build passed; the explicit native credential test was not rerun.
- Four packaging tests and two Skill tests passed, including the documented DuckDB computation and result limits. Five publishing scenarios passed with mocked `gh`; no GitHub calls were made by those tests.
- `actionlint` 1.7.12 accepted the workflows. A real macOS ARM64 CLI archive and matching Skill ZIP were created locally, and the extracted CLI passed version/help/configuration checks.
- No branch or tag was pushed, no GitHub-hosted workflow was run, and no Release was published. The other four target jobs, downloads from a real Release, platform signing, fresh-machine installation and native credential behavior still require their own verification.

## Hosted CI and artifact validation (2026-09-09)

The first run at `34c6bce` passed both macOS targets, both Linux targets, and the Skill job. Windows failed Clippy because a mutable `DirBuilder` was only mutated in Unix-specific code. Commit `90ccde0` scopes that mutable binding to Unix without changing directory creation or Unix permissions.

[Run 34300891337](https://github.com/logcove/logcove-public/actions/runs/34300891337), at commit `90ccde07735417e09be0e5f622ecda447cb4917d`, completed successfully:

| Job | Result |
| --- | --- |
| macOS ARM64 | Rust checks/tests, release build, packaging and extracted CLI smoke checks passed |
| macOS x64 | Rust checks/tests, release build, packaging and extracted CLI smoke checks passed |
| Linux ARM64 | Rust checks/tests, release build, packaging and extracted CLI smoke checks passed |
| Linux x64 | Rust checks/tests, release build, packaging and extracted CLI smoke checks passed |
| Windows x64 MSVC | Rust checks/tests, release build, packaging and extracted CLI smoke checks passed |
| Skill and release checks | Skill metadata/examples, simulated publishing scenarios, and Skill ZIP upload passed |

All six actual Actions archives were downloaded locally. The release script accepted the complete asset set, generated SHA256SUMS, and all six hashes were verified. Each CLI archive was extracted and checked for its executable, license and matching Skill content, normalizing Windows checkout line endings for text comparisons. The separate Skill ZIP's version and reference files matched the source. The downloaded macOS ARM64 executable also ran version and command-help checks locally.

Artifacts and the local report are retained under ignored `experiments/ci-validation/34300891337/`. These are Actions artifacts, not GitHub Release downloads. The tag-triggered publishing API flow remains simulated, and no tag or Release was created. Browser authorization, native credential persistence on Windows/Linux, signing/notarization, installers and fresh-user installation remain separate acceptance work.
