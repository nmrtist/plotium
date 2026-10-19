"""Cargo is the only release-version source, including Apple bundle fields."""
import json
import os
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[2]
NUMBER = r"(?:0|[1-9][0-9]*)"
PATTERN = re.compile(rf"({NUMBER})\.({NUMBER})\.({NUMBER})(?:-(alpha|beta)\.({NUMBER}))?")


def parse(version):
    match = PATTERN.fullmatch(version)
    if not match:
        raise ValueError("Use X.Y.Z, X.Y.Z-alpha.N, or X.Y.Z-beta.N")
    return match


def cargo_version():
    metadata = json.loads(subprocess.check_output([
        "cargo", "metadata", "--locked", "--no-deps", "--format-version", "1",
    ], cwd=ROOT))
    version = next(p["version"] for p in metadata["packages"] if p["name"] == "plotium")
    parse(version)
    return version


def apple_versions(version):
    match = parse(version)
    major, minor, patch = map(int, match.group(1, 2, 3))
    channel, number = match.group(4, 5)
    number = int(number or 0)
    if number > 99999:
        raise ValueError("Apple packages support prerelease numbers 0 through 99999")
    stage = {"alpha": 0, "beta": 100000, None: 200000}[channel]
    build_patch = patch * 1000000 + stage + number
    if max(major, minor, build_patch) > 2147483647:
        raise ValueError("Apple bundle version component exceeds a signed 32-bit integer")
    return f"{major}.{minor}.{patch}", f"{major}.{minor}.{build_patch}"


if __name__ == "__main__":
    version = cargo_version()
    tag = os.environ.get("TAG", "")
    if tag and tag != f"v{version}":
        raise SystemExit(f"Tag must match Cargo.toml: expected v{version}")
    values = f"version={version}\nchannel={parse(version).group(4) or 'stable'}\n"
    if "GITHUB_OUTPUT" in os.environ:
        with open(os.environ["GITHUB_OUTPUT"], "a") as output:
            output.write(values)
    print(values, end="")
