"""Generate Homebrew and WinGet metadata from actual release archives."""

import argparse
import hashlib
from pathlib import Path
import re
import tempfile

from release import ROOT, TARGETS, archive_tree, cli_name, version


REPOSITORY = "https://github.com/logcove/logcove-public"
PACKAGE_ID = "Logcove.Logcove"


def generate(value, assets, output):
    if not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?", value):
        raise ValueError("Use a release version such as 0.2.0 or 0.2.0-rc.1")
    hashes = {}
    for target in TARGETS:
        with (assets / cli_name(value, target)).open("rb") as handle:
            hashes[target] = hashlib.file_digest(handle, "sha256").hexdigest()
    base = f"{REPOSITORY}/releases/download/v{value}"
    formula = [
        "class Logcove < Formula",
        '  desc "Analyze Logcove logs and manage charts from your terminal"',
        '  homepage "https://logcove.com"',
        f'  version "{value}"',
        '  license "Apache-2.0"',
    ]
    for os_name, suffix in (("macos", "apple-darwin"), ("linux", "unknown-linux-gnu")):
        formula.append(f"\n  on_{os_name} do")
        for architecture, cpu in (("aarch64", "arm"), ("x86_64", "intel")):
            target = f"{architecture}-{suffix}"
            formula.extend([
                f"    on_{cpu} do",
                f'      url "{base}/{cli_name(value, target)}"',
                f'      sha256 "{hashes[target]}"',
                "    end",
            ])
        formula.append("  end")
    formula.extend([
        "", "  def install", '    bin.install "logcove"', "  end", "",
        "  test do", f'    assert_match "logcove {value}", shell_output("#{{bin}}/logcove --version")',
        "  end", "end", "",
    ])
    windows = "x86_64-pc-windows-msvc"
    common = f'PackageIdentifier: {PACKAGE_ID}\nPackageVersion: "{value}"\n'
    manifests = {
        f"{PACKAGE_ID}.yaml": common + "DefaultLocale: en-US\nManifestType: version\nManifestVersion: 1.9.0\n",
        f"{PACKAGE_ID}.locale.en-US.yaml": common + (
            "PackageLocale: en-US\nPublisher: Logcove\nPackageName: Logcove\n"
            f"PackageUrl: {REPOSITORY}\nLicense: Apache-2.0\n"
            f"LicenseUrl: {REPOSITORY}/blob/v{value}/LICENSE\n"
            "ShortDescription: Analyze Logcove logs and manage charts from your terminal\n"
            "ManifestType: defaultLocale\nManifestVersion: 1.9.0\n"
        ),
        f"{PACKAGE_ID}.installer.yaml": common + (
            "InstallerType: zip\nNestedInstallerType: portable\n"
            "NestedInstallerFiles:\n"
            f"- RelativeFilePath: logcove-v{value}-{windows}/logcove.exe\n"
            "  PortableCommandAlias: logcove\nScope: user\nCommands:\n- logcove\n"
            "Installers:\n- Architecture: x64\n"
            f"  InstallerUrl: {base}/{cli_name(value, windows)}\n"
            f"  InstallerSha256: {hashes[windows].upper()}\n"
            "ManifestType: installer\nManifestVersion: 1.9.0\n"
        ),
    }
    formula_path = output / "Formula/logcove.rb"
    formula_path.parent.mkdir(parents=True, exist_ok=True)
    formula_path.write_text("\n".join(formula), encoding="utf-8")
    manifest_directory = output / "manifests/l/Logcove/Logcove" / value
    manifest_directory.mkdir(parents=True, exist_ok=True)
    for name, text in manifests.items():
        (manifest_directory / name).write_text(text, encoding="utf-8")
    (output / "README.md").write_text(
        f"# Logcove {value} package-manager metadata\n\n"
        "Generated from the corresponding CLI archives. URLs and SHA-256 hashes\n"
        "refer to those exact release assets. Generation does not publish to a package index.\n\n"
        "Publish Formula/logcove.rb in logcove/homebrew-tap,\n"
        "then users can run `brew install logcove/tap/logcove`. Submit the manifests/\n"
        "directory to microsoft/winget-pkgs for review; after acceptance users can run\n"
        "`winget install --id Logcove.Logcove --exact --source winget`.\n\n"
        "Both channels require anonymously downloadable release URLs. Verify those URLs\n"
        "and test installation before announcing package-manager availability.\n",
        encoding="utf-8",
    )
    return output


def package(value, assets):
    destination = assets / f"logcove-package-managers-v{value}.zip"
    with tempfile.TemporaryDirectory() as temporary:
        source = generate(value, assets, Path(temporary) / "package-managers")
        archive_tree(source, destination)
    return destination


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--assets", type=Path, default=ROOT / "dist")
    parser.add_argument("--version", default=version())
    args = parser.parse_args()
    print(package(args.version, args.assets))
