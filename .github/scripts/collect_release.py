"""Require the complete release asset set, then generate its checksum manifest."""
import argparse
import hashlib
from pathlib import Path

from package_portable import PLATFORMS
from release_version import parse


def expected_assets(version):
    parse(version)
    return sorted(
        [f"plotium-{version}-{p}.zip" for p in PLATFORMS.values()]
        + [f"plotium-{version}-{p}.dmg" for p in PLATFORMS.values() if p.startswith("macos-")]
    )


def collect(directory, version):
    expected = expected_assets(version)
    actual = sorted(p.name for p in directory.iterdir() if p.suffix in (".zip", ".dmg"))
    if actual != expected:
        raise ValueError(f"Release assets differ: expected {expected}, got {actual}")
    lines = []
    for name in expected:
        path = directory / name
        if not path.is_file() or path.stat().st_size == 0:
            raise ValueError(f"Release asset is empty or not a file: {name}")
        digest = hashlib.sha256()
        with path.open("rb") as source:
            for chunk in iter(lambda: source.read(1024 * 1024), b""):
                digest.update(chunk)
        lines.append(f"{digest.hexdigest()}  {name}\n")
    (directory / "SHA256SUMS").write_text("".join(lines), encoding="utf-8")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    parser.add_argument("version")
    args = parser.parse_args()
    collect(args.directory, args.version)
