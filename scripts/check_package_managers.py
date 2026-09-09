"""Check generated package metadata against archives and official WinGet schemas."""

import argparse
import hashlib
from pathlib import Path
import re
import tempfile
import urllib.request
import json
import zipfile

import jsonschema
import yaml

from package_managers import PACKAGE_ID, REPOSITORY
from release import TARGETS, cli_name, extract_archive, version


def check(value, assets):
    with tempfile.TemporaryDirectory() as temporary:
        root = Path(temporary)
        extract_archive(assets / f"logcove-package-managers-v{value}.zip", root)
        root = root / "package-managers"
        formula = (root / "Formula/logcove.rb").read_text()
        entries = re.findall(r'url "([^"]+)"\s+sha256 "([^"]+)"', formula)
        expected = {}
        for target in TARGETS:
            name = cli_name(value, target)
            with (assets / name).open("rb") as handle:
                expected[f"{REPOSITORY}/releases/download/v{value}/{name}"] = hashlib.file_digest(handle, "sha256").hexdigest()
        assert len(entries) == 4
        for url, digest in entries:
            assert expected[url] == digest
        manifests = root / "manifests/l/Logcove/Logcove" / value
        assert len(list(manifests.glob("*.yaml"))) == 3
        for file in manifests.glob("*.yaml"):
            manifest = yaml.safe_load(file.read_text())
            kind = manifest["ManifestType"]
            schema_url = f"https://raw.githubusercontent.com/microsoft/winget-cli/master/schemas/JSON/manifests/v1.9.0/manifest.{kind}.1.9.0.json"
            with urllib.request.urlopen(schema_url, timeout=30) as response:
                schema = json.load(response)
            jsonschema.validate(manifest, schema)
            assert manifest["PackageIdentifier"] == PACKAGE_ID
            assert manifest["PackageVersion"] == value
            if kind == "installer":
                entry, = manifest["Installers"]
                assert expected[entry["InstallerUrl"]] == entry["InstallerSha256"].lower()
                nested, = manifest["NestedInstallerFiles"]
                with zipfile.ZipFile(assets / cli_name(value, "x86_64-pc-windows-msvc")) as archive:
                    assert nested["RelativeFilePath"] in archive.namelist()
                assert nested["PortableCommandAlias"] == "logcove"
    print("Homebrew URLs/hashes and WinGet schemas/archive paths passed")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--assets", type=Path, required=True)
    parser.add_argument("--version", default=version())
    args = parser.parse_args()
    check(args.version, args.assets)
