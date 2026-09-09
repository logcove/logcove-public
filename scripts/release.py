"""Build release archives from explicit inputs; no network or credentials required."""

import argparse
import hashlib
import json
from pathlib import Path
import re
import shlex
import shutil
import subprocess
import tarfile
import tempfile
import tomllib
import zipfile


ROOT = Path(__file__).resolve().parents[1]
TARGETS = (
    "aarch64-apple-darwin",
    "x86_64-apple-darwin",
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
    "x86_64-pc-windows-msvc",
)
SKILL_FILES = (
    "SKILL.md",
    "references/cli.md",
    "references/duckdb.md",
    "references/charts.md",
)


def version(tag=None):
    value = tomllib.loads((ROOT / "cli/Cargo.toml").read_text())["package"]["version"]
    if tag is not None and tag != f"v{value}":
        raise ValueError(f"Tag {tag!r} must match Cargo version v{value}")
    return value


def cli_name(value, target):
    suffix = "zip" if target.endswith("windows-msvc") else "tar.gz"
    return f"logcove-v{value}-{target}.{suffix}"


def archive_tree(source, destination):
    files = sorted(path for path in source.rglob("*") if path.is_file())
    if destination.suffix == ".zip":
        with zipfile.ZipFile(destination, "w", zipfile.ZIP_DEFLATED) as archive:
            for path in files:
                archive.write(path, path.relative_to(source.parent).as_posix())
    else:
        with tarfile.open(destination, "w:gz") as archive:
            for path in files:
                archive.add(path, arcname=path.relative_to(source.parent).as_posix())


def extract_archive(archive, destination):
    if archive.suffix == ".zip":
        with zipfile.ZipFile(archive) as handle:
            handle.extractall(destination)
    else:
        with tarfile.open(archive) as handle:
            handle.extractall(destination, filter="data")


def copy_skill(destination):
    for relative in SKILL_FILES:
        path = destination / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT / "skills/logcove" / relative, path)


def package_cli(value, target, output):
    executable = "logcove.exe" if target.endswith("windows-msvc") else "logcove"
    binary = ROOT / "target" / target / "release" / executable
    name = f"logcove-v{value}-{target}"
    archive = output / cli_name(value, target)
    with tempfile.TemporaryDirectory() as temporary:
        staging = Path(temporary) / name
        staging.mkdir()
        shutil.copy2(binary, staging / executable)
        for document in ("LICENSE", "README.md", "AGENTS.md"):
            shutil.copy2(ROOT / document, staging / document)
        (staging / "docs").mkdir()
        for document in (
            "cli.md", "skills.md", "releases.md", "validation.md",
            "skill-validation.md", "implementation-plan.md", "development.md",
        ):
            shutil.copy2(ROOT / "docs" / document, staging / "docs" / document)
        copy_skill(staging / "skills/logcove")
        (staging / "README.txt").write_text(
            f"Logcove {value} ({target})\n\n"
            "Place logcove (logcove.exe on Windows) in a directory on your PATH.\n"
            "Run logcove --version to verify installation.\n"
            "Configure your deployed API with logcove config set-api-url <origin>,\n"
            "then run logcove login. No default API is built into this version.\n"
            "See README.md for installation and quick start; docs/ has detailed guides.\n"
            "DuckDB is installed separately. Linux login requires an unlocked\n"
            "Secret Service. These archives are not code-signed or notarized.\n",
            encoding="utf-8",
        )
        archive_tree(staging, archive)
        extracted = Path(temporary) / "extracted"
        extract_archive(archive, extracted)
        installed = extracted / name / executable
        actual = subprocess.check_output([str(installed), "--version"], text=True).strip()
        if actual != f"logcove {value}":
            raise ValueError(f"Binary version mismatch: {actual}")
        subprocess.run([str(installed), "data", "pull", "--help"], check=True, stdout=subprocess.DEVNULL)
        for relative in SKILL_FILES:
            text = (ROOT / "skills/logcove" / relative).read_text(encoding="utf-8")
            for block in re.findall(r"```sh\n(.*?)\n```", text, re.DOTALL):
                for line in block.replace("\\\n", " ").splitlines():
                    arguments = shlex.split(line)
                    if arguments and arguments[0] == "logcove":
                        # Clap exits for help before credentials, file reads, or API calls.
                        subprocess.run(
                            [str(installed), *arguments[1:], "--help"],
                            check=True, stdout=subprocess.DEVNULL,
                        )
        # A separate config directory keeps smoke checks independent of local login state.
        result = subprocess.check_output(
            [str(installed), "--config-dir", str(Path(temporary) / "config"), "config", "show"],
            text=True,
        )
        json.loads(result)["data"]
    return archive


def package_skill(value, output):
    archive = output / f"logcove-skills-v{value}.zip"
    with tempfile.TemporaryDirectory() as temporary:
        staging = Path(temporary) / "logcove"
        copy_skill(staging)
        shutil.copy2(ROOT / "LICENSE", staging / "LICENSE")
        (staging / "VERSION").write_text(value + "\n", encoding="utf-8")
        archive_tree(staging, archive)
    return archive


def checksums(value, output):
    expected = {cli_name(value, target) for target in TARGETS}
    expected.add(f"logcove-skills-v{value}.zip")
    actual = {path.name for path in output.iterdir() if path.name != "SHA256SUMS"}
    if actual != expected:
        raise ValueError(f"Incomplete or unexpected assets: missing={expected - actual}, extra={actual - expected}")
    lines = []
    for name in sorted(expected):
        with (output / name).open("rb") as handle:
            digest = hashlib.file_digest(handle, "sha256").hexdigest()
        lines.append(f"{digest}  {name}\n")
    path = output / "SHA256SUMS"
    path.write_text("".join(lines), encoding="utf-8")
    return path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("version").add_argument("--tag")
    cli = commands.add_parser("cli")
    cli.add_argument("--target", choices=TARGETS, required=True)
    for command in (cli, commands.add_parser("skill"), commands.add_parser("checksums")):
        command.add_argument("--output", type=Path, default=ROOT / "dist")
    args = parser.parse_args()
    value = version(getattr(args, "tag", None))
    if args.command == "version":
        print(value)
        return
    args.output.mkdir(parents=True, exist_ok=True)
    if args.command == "cli":
        result = package_cli(value, args.target, args.output)
    elif args.command == "skill":
        result = package_skill(value, args.output)
    else:
        result = checksums(value, args.output)
    print(result)


if __name__ == "__main__":
    main()
