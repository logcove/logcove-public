# CLI and Skill releases

## Status and scope

Batch 1 distribution automation is implemented: shared CI checks, five native build targets, CLI/Skill archives, SHA-256 checksums, and a tag-triggered GitHub Release workflow. No release has been published as part of this implementation. Local checks do not prove that hosted jobs or all platform runtimes have passed; the first GitHub run must establish that evidence.

Batch 2 remains planned: installation/update scripts and installation on fresh user environments. There is no self-update command, package-manager release, automatic Skill updater, bundled DuckDB, or code signing/notarization in batch 1.

The CLI still requires an explicit API origin. The agreed default `https://api.logcove.com` is a future runtime change after deployment and verification. Before an external test release, provide a working HTTPS test service and instructions for configuring it. A stable public release also requires the production service and default-origin change.

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

## Checks and packaging

`.github/workflows/ci.yml` runs on branch pushes, pull requests, and manual dispatch. It calls `checks.yml`, which is also reused by `release.yml` at the tagged commit:

- All five targets: rustfmt, Clippy with warnings denied, Rust unit/contract tests, release build, and Python packaging tests.
- Each CLI archive is extracted and the extracted executable runs version, command help, and isolated configuration checks. Shell examples in Skill references are checked through Clap's help handling; no example API operations run.
- The Skill job validates YAML metadata and internal references, then executes the actual DuckDB reference snippets against temporary synthetic Parquet files. Checks cover exact manifest files, schema evolution, aggregates, result fields, empty results versus no files, and row/size/alias/NaN rejection.
- Packaging tests cover tag mismatch, archive contents and extraction, executable permissions for tar archives, and rejecting incomplete or unexpected release asset sets.
- The publishing shell block is tested with a fake `gh` command: first publish, draft retry, refusing a public release overwrite, prerelease marking, and upload failure preventing publication. This is simulation, not a real GitHub release test.

CI uses Python 3.12. Packaging uses only its standard library; Skill checks additionally install pinned DuckDB and PyYAML from `scripts/requirements-ci.txt`. These are CI dependencies, not CLI runtime dependencies. No test account, R2 credentials, OAuth secrets, or LLM access is required. These checks do not render Vega charts or validate an independent agent conversation.

Branch and PR runs upload the same package format as Actions artifacts, retained for seven days. They are development artifacts, not published releases. `dist/` is ignored by Git. Packaging copies an explicit file list rather than arbitrary repository contents or local experiments.

## Release assets

For version `0.1.0`, a complete release has six archives plus one checksum file:

```text
logcove-v0.1.0-aarch64-apple-darwin.tar.gz
logcove-v0.1.0-x86_64-apple-darwin.tar.gz
logcove-v0.1.0-x86_64-unknown-linux-gnu.tar.gz
logcove-v0.1.0-aarch64-unknown-linux-gnu.tar.gz
logcove-v0.1.0-x86_64-pc-windows-msvc.zip
logcove-skills-v0.1.0.zip
SHA256SUMS
```

Each CLI archive has a `logcove-v<version>-<target>/` root containing its executable, LICENSE, README.txt, matching public documentation and the small `skills/logcove/` source folder. This keeps the bundled documentation and Skill installation instructions usable after extraction. The separate Skill ZIP supports users who only need the Skill; it contains a directly installable `logcove/` folder with SKILL.md, all three references, LICENSE and VERSION. Install the complete folder using [Skill installation](skills.md), or follow the source-install instructions until a release exists.

Check downloaded archives against SHA256SUMS before use (`sha256sum` on Linux, `shasum -a 256` on macOS, or `Get-FileHash -Algorithm SHA256` on Windows). These hashes detect file mismatch; they are not a substitute for platform code signing. Downloaded unsigned executables may have a different OS launch experience from local builds; fresh-machine verification and signing decisions remain batch 2 work.

## Publishing procedure

1. Update the shared Cargo version and lockfile, review changes and release notes, and commit them. Check the hosted service prerequisites above before publishing externally.
2. Explicitly authorize and push the matching version tag. Branch pushes and manual CI runs never publish releases.
3. `release.yml` validates the tag before running the shared checks at that commit. Every target and the Skill job must pass.
4. The publish job downloads artifacts from that workflow run, requires exactly the six expected archives, and writes SHA256SUMS. Only this job gets `contents: write` via GitHub's built-in token; no personal access token is needed.
5. Create a draft Release with generated notes, upload all assets, then make it public. A failed upload leaves a draft; rerunning the job can replace assets in that draft. An already public Release is never overwritten by this workflow. To correct a published version, publish a new version.

Only jobs within the same run supply release artifacts. Do not move or recreate release tags; fix failures through a workflow rerun on the unchanged tag or a new version. Generated notes summarize merged changes; maintainers should describe user-visible changes and known limitations in commit/PR descriptions.

The brief draft phase is an upload staging step, not another manual approval gate. Pushing the version tag is the publishing action. This implementation alone does not authorize pushing tags or publishing any release.

## Local verification

From the repository root, using Python 3.12 or newer:

```sh
python3 -m unittest discover -s scripts/tests -v
python3 scripts/release.py version --tag v0.1.0
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

`python3 scripts/release.py checksums` intentionally fails until `dist/` contains exactly all five CLI archives and the matching Skill ZIP. Do not use placeholder packages for an actual release.

## Batch 1 local validation (2026-09-08)

- macOS ARM64 with Rust 1.90.0: rustfmt, Clippy, all 54 Rust tests and target-specific release build passed; the explicit native credential test was not rerun.
- Four packaging tests and two Skill tests passed, including the documented DuckDB computation and result limits. Five publishing scenarios passed with mocked `gh`; no GitHub calls were made by those tests.
- `actionlint` 1.7.12 accepted the workflows. A real macOS ARM64 CLI archive and matching Skill ZIP were created locally, and the extracted CLI passed version/help/configuration checks.
- No branch or tag was pushed, no GitHub-hosted workflow was run, and no Release was published. The other four target jobs, downloads from a real Release, platform signing, fresh-machine installation and native credential behavior still require their own verification.
