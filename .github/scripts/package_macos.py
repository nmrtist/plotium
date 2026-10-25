"""Create an ad-hoc-signed Plotium.app and a native drag-install DMG on macOS."""
import argparse
import json
import os
from pathlib import Path
import plistlib
import re
import shutil
import subprocess
import sys
import tempfile

from dmg_layout import validate_layout, write_layout
import package_mcp
from package_portable import PLATFORMS, stage, run
from release_version import ROOT, apple_versions

TARGETS = {"aarch64-apple-darwin": "arm64", "x86_64-apple-darwin": "x86_64"}
IDENTIFIER = "space.nmrtist.plotium"
RESOURCES = (
    "LICENSE", "COPYRIGHT.md", "README.md", "THIRD-PARTY-LICENSES.html",
    "MCP-THIRD-PARTY-LICENSES.html", "plotium.mcpb",
)


def info_plist(version):
    short, build = apple_versions(version)
    project_type = "space.nmrtist.plotium.project"
    return {
        "CFBundleInfoDictionaryVersion": "6.0",
        "CFBundlePackageType": "APPL",
        "CFBundleName": "Plotium",
        "CFBundleDisplayName": "Plotium",
        "CFBundleIdentifier": IDENTIFIER,
        "CFBundleExecutable": "plotium",
        "CFBundleIconFile": "Plotium.icns",
        "CFBundleShortVersionString": short,
        "CFBundleVersion": build,
        "PlotiumCargoVersion": version,
        "LSMinimumSystemVersion": "13.0",
        "NSHighResolutionCapable": True,
        "NSHumanReadableCopyright": "Copyright © 2026 Plotium contributors. GPL-3.0-or-later.",
        "CFBundleDocumentTypes": [{
            "CFBundleTypeName": "Plotium project", "CFBundleTypeRole": "Editor",
            "LSHandlerRank": "Owner", "LSItemContentTypes": [project_type],
        }],
        "UTExportedTypeDeclarations": [{
            "UTTypeIdentifier": project_type, "UTTypeDescription": "Plotium project",
            "UTTypeConformsTo": ["public.data"],
            "UTTypeTagSpecification": {"public.filename-extension": ["plotium"]},
        }],
    }


def inspect_binary(binary, architecture):
    if not binary.is_file() or not os.access(binary, os.X_OK):
        raise ValueError(f"Missing executable or execute permission: {binary}")
    mode = binary.stat().st_mode & 0o7777
    if mode != 0o755:
        raise ValueError(f"Expected executable mode 0755, got {mode:o}: {binary}")
    arches = subprocess.check_output(["lipo", "-archs", str(binary)], text=True).strip()
    if arches != architecture:
        raise ValueError(f"Expected only {architecture}, got {arches}: {binary}")
    dependencies = subprocess.check_output(["otool", "-L", str(binary)], text=True)
    for line in dependencies.splitlines()[1:]:
        dependency = line.strip().split(" (", 1)[0]
        if not dependency.startswith(("/System/Library/", "/usr/lib/")):
            raise ValueError(f"Non-system runtime dependency in {binary}: {dependency}")
    commands = subprocess.check_output(["otool", "-l", str(binary)], text=True)
    minimum = re.search(r"\bminos\s+(\d+\.\d+(?:\.\d+)?)", commands)
    if not minimum or tuple(map(int, minimum[1].split(".")[:2])) != (13, 0):
        raise ValueError(f"Expected MACOSX_DEPLOYMENT_TARGET=13.0: {binary}")
    return {"architectures": arches, "mode": f"{mode:04o}",
            "minimum_macos": minimum[1], "dependencies": dependencies}


def validate_app(app, target, version):
    contents = app / "Contents"
    with (contents / "Info.plist").open("rb") as file:
        info = plistlib.load(file)
    if info != info_plist(version):
        raise ValueError("Packaged Info.plist does not match the Cargo release metadata")
    run("plutil", "-lint", contents / "Info.plist")
    resources = contents / "Resources"
    for name in (*RESOURCES, "Plotium.icns"):
        if not (resources / name).is_file() or (resources / name).stat().st_size == 0:
            raise ValueError(f"Missing bundle resource: {name}")
    # iconutil must be able to decode the shipped ICNS, not only its source PNGs.
    with tempfile.TemporaryDirectory() as temporary:
        iconset = Path(temporary) / "verify.iconset"
        run("iconutil", "-c", "iconset", "-o", iconset, resources / "Plotium.icns")
        for size in (16, 32, 128, 256, 512):
            for retina in (False, True):
                png = iconset / f"icon_{size}x{size}{'@2x' if retina else ''}.png"
                data = png.read_bytes()
                expected = size * (2 if retina else 1)
                if (int.from_bytes(data[16:20], "big"), int.from_bytes(data[20:24], "big")) != (expected, expected):
                    raise ValueError(f"Wrong ICNS representation size: {png.name}")
    binaries = {
        name: inspect_binary(contents / "MacOS" / name, TARGETS[target])
        for name in ("plotium", "plotium-mcp")
    }
    run("codesign", "--verify", "--deep", "--strict", app)
    return binaries


def security_evidence(app, dmg):
    signature = subprocess.run(["codesign", "-dv", "--verbose=2", str(app)],
                               capture_output=True, text=True, check=True)
    assessment = subprocess.run(["spctl", "--assess", "--type", "execute", "--verbose=2", str(app)],
                                capture_output=True, text=True)
    quarantine = subprocess.run(["xattr", "-p", "com.apple.quarantine", str(app)],
                               capture_output=True, text=True)
    dmg_quarantine = subprocess.run(["xattr", "-p", "com.apple.quarantine", str(dmg)],
                                   capture_output=True, text=True)
    return {
        "signature_kind": "ad-hoc", "signature_details": signature.stderr,
        "notarized": False, "stapled": False,
        "dmg_signature_kind": "unsigned",
        "gatekeeper_exit_code": assessment.returncode,
        "gatekeeper_output": assessment.stdout + assessment.stderr,
        "quarantine": quarantine.stdout.strip() if quarantine.returncode == 0 else None,
        "quarantine_probe_exit_code": quarantine.returncode,
        "quarantine_probe_stderr": quarantine.stderr,
        "dmg_quarantine": dmg_quarantine.stdout.strip() if dmg_quarantine.returncode == 0 else None,
        "dmg_quarantine_probe_exit_code": dmg_quarantine.returncode,
        "dmg_quarantine_probe_stderr": dmg_quarantine.stderr,
        "download_install_test": "not performed; local build/mount/copy is not an Internet download",
    }


def create_dmg(staging, dmg):
    with tempfile.TemporaryDirectory(prefix="plotium-dmg-build-") as temporary:
        temporary = Path(temporary)
        artwork = temporary / "artwork"
        run("cargo", "run", "--locked", "-q", "-p", "xtask", "--features", "macos-packaging",
            "--", "macos-dmg-background", artwork)
        tiffs = []
        for name, dpi in [("background", 72), ("background@2x", 144)]:
            tiff = artwork / f"{name}.tiff"
            run("sips", "-s", "format", "tiff", "-s", "dpiWidth", dpi, "-s", "dpiHeight", dpi,
                artwork / f"{name}.png", "--out", tiff)
            tiffs.append(tiff)
        run("tiffutil", "-cathidpicheck", *tiffs, "-out", staging / ".background.tiff")
        writable = temporary / "writable.dmg"
        run("hdiutil", "create", "-volname", "Plotium", "-srcfolder", staging,
            "-fs", "HFS+", "-format", "UDRW", writable)
        mount = temporary / "volume"
        mount.mkdir()
        run("hdiutil", "attach", "-nobrowse", "-mountpoint", mount, writable)
        try:
            # The Finder background alias must reference the image filesystem,
            # not the staging directory on the build machine.
            write_layout(mount)
            validate_layout(mount)
        finally:
            run("hdiutil", "detach", mount)
        run("hdiutil", "convert", writable, "-format", "UDZO", "-ov", "-o", dmg)


def create(target, output, skip_build=False, skip_connector_smoke=False):
    if sys.platform != "darwin":
        raise RuntimeError("macOS packaging requires macOS and Apple command-line tools")
    directory, version = stage(target, output, skip_build)
    platform = PLATFORMS[target]
    package_mcp.package(directory, platform, version, runtime_smoke=not skip_connector_smoke)
    staging = output / f"dmg-{platform}"
    if staging.exists():
        shutil.rmtree(staging)
    app = staging / "Plotium.app"
    macos = app / "Contents/MacOS"
    resources = app / "Contents/Resources"
    macos.mkdir(parents=True)
    resources.mkdir()
    for name in ("plotium", "plotium-mcp"):
        shutil.copy2(directory / name, macos / name)
        (macos / name).chmod(0o755)
    for name in RESOURCES:
        shutil.copy2(directory / name, resources / name)
    with (app / "Contents/Info.plist").open("wb") as file:
        plistlib.dump(info_plist(version), file)
    run("cargo", "run", "--locked", "-q", "-p", "xtask", "--features", "macos-packaging",
        "--", "macos-icon", resources / "Plotium.icns")
    # Sign nested code first; sign the bundle last after all resources are final.
    run("codesign", "--force", "--sign", "-", "--timestamp=none", macos / "plotium-mcp")
    run("codesign", "--force", "--sign", "-", "--timestamp=none", "--identifier", IDENTIFIER, app)
    validate_app(app, target, version)
    (staging / "Applications").symlink_to("/Applications", target_is_directory=True)
    dmg = output / f"plotium-{version}-{platform}.dmg"
    create_dmg(staging, dmg)
    run("hdiutil", "verify", dmg)
    verified = output / f"installed-{platform}"
    if verified.exists():
        shutil.rmtree(verified)
    verified.mkdir()
    with tempfile.TemporaryDirectory(prefix="plotium-dmg-") as temporary:
        mount = Path(temporary) / "volume"
        mount.mkdir()
        run("hdiutil", "attach", "-readonly", "-nobrowse", "-mountpoint", mount, dmg)
        try:
            finder_layout = validate_layout(mount)
            if os.readlink(mount / "Applications") != "/Applications":
                raise ValueError("DMG is missing its Applications drag-install shortcut")
            if sorted(p.name for p in mount.iterdir() if not p.name.startswith(".")) != ["Applications", "Plotium.app"]:
                raise ValueError("Unexpected DMG root contents")
            run("ditto", mount / "Plotium.app", verified / "Plotium.app")
        finally:
            run("hdiutil", "detach", mount)
    installed = verified / "Plotium.app"
    report = {"version": version, "target": target, "dmg": str(dmg),
              "installed_copy": str(installed), "runtime_test": "not performed by packaging script",
              "connector_smoke": "not performed (--skip-connector-smoke)" if skip_connector_smoke else "passed",
              "finder_layout": finder_layout,
              "binaries": validate_app(installed, target, version),
              "security": security_evidence(installed, dmg)}
    (output / f"macos-validation-{platform}.json").write_text(
        json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(f"Created {dmg}\nVerified installed copy: {installed}\nSignature: ad-hoc; not notarized.")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", choices=TARGETS, required=True)
    parser.add_argument("--output", type=Path, default=ROOT / "dist")
    parser.add_argument("--skip-build", action="store_true")
    parser.add_argument("--skip-connector-smoke", action="store_true",
                        help="Cross-packaging only: record connector runtime checks as not performed")
    args = parser.parse_args()
    create(args.target, args.output.resolve(), args.skip_build, args.skip_connector_smoke)
