"""Build the existing flat ZIP contract; macOS also reuses its staged files."""
import argparse
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import zipfile

import package_mcp
from release_version import ROOT, cargo_version, parse

PLATFORMS = {
    "x86_64-pc-windows-msvc": "windows-x86_64",
    "x86_64-unknown-linux-gnu": "linux-x86_64",
    "aarch64-apple-darwin": "macos-aarch64",
    "x86_64-apple-darwin": "macos-x86_64",
}


def run(*args, **kwargs):
    print("+", " ".join(map(str, args)), flush=True)
    return subprocess.run(list(map(str, args)), cwd=ROOT, check=True, **kwargs)


def package(target, output, skip_build=False, skip_connector_smoke=False):
    version = cargo_version()
    platform = PLATFORMS[target]
    environment = dict(os.environ, PLOTIUM_RELEASE_CHANNEL=parse(version).group(4) or "stable")
    if platform.startswith("macos-"):
        environment["MACOSX_DEPLOYMENT_TARGET"] = "13.0"
    if not skip_build:
        run("cargo", "build", "--locked", "--release", "--target", target,
            "-p", "plotium", "-p", "plotium-agent", env=environment)
    output.mkdir(parents=True, exist_ok=True)
    directory = output / f"package-{platform}"
    # Each attempt gets a fresh staging directory so an old release companion
    # cannot silently leak into a new package.
    if directory.exists():
        shutil.rmtree(directory)
    directory.mkdir()
    for manifest, notice in [
        ("crates/app/Cargo.toml", "THIRD-PARTY-LICENSES.html"),
        ("crates/agent/Cargo.toml", "MCP-THIRD-PARTY-LICENSES.html"),
    ]:
        run("cargo", "about", "generate", "--locked", "--fail", "--target", target,
            "--manifest-path", manifest, "-c", "xtask/about.toml", "xtask/about.hbs",
            "-o", directory / notice)
    suffix = ".exe" if platform.startswith("windows-") else ""
    target_dir = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target"))
    if not target_dir.is_absolute():
        target_dir = ROOT / target_dir
    for name in ("plotium", "plotium-mcp"):
        shutil.copy2(target_dir / target / "release" / (name + suffix), directory)
    for name in ("LICENSE", "COPYRIGHT.md", "README.md"):
        shutil.copy2(ROOT / name, directory)
    package_mcp.package(directory, platform, version, runtime_smoke=not skip_connector_smoke)
    archive = output / f"plotium-{version}-{platform}.zip"
    with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as zipped:
        for path in sorted(directory.iterdir()):
            zipped.write(path, path.name)
    with zipfile.ZipFile(archive) as zipped, tempfile.TemporaryDirectory() as smoke:
        if zipped.testzip() is not None:
            raise RuntimeError("Release ZIP failed its CRC check")
        zipped.extractall(smoke)
        connector = Path(smoke) / ("plotium-mcp" + suffix)
        connector.chmod(0o755)
        if not skip_connector_smoke:
            package_mcp.smoke(connector)
    return directory, version


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", choices=PLATFORMS, required=True)
    parser.add_argument("--output", type=Path, default=ROOT / "dist")
    parser.add_argument("--skip-build", action="store_true")
    args = parser.parse_args()
    package(args.target, args.output.resolve(), args.skip_build)
