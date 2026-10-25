"""Offline regression checks for release metadata and packaging contracts."""
from datetime import datetime, timezone
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import zipfile

from collect_release import collect, expected_assets
import package_portable
import package_mcp
from package_macos import info_plist, inspect_binary
from release_version import apple_versions, parse
from dmg_layout import POSITIONS, background_alias, validate_layout, write_layout
from ds_store import DSStore
from mac_alias import Alias, TargetInfo, VolumeInfo


class ReleaseTests(unittest.TestCase):
    def test_background_alias_resolves_symlinked_temporary_mount_paths(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            volume = directory / "volume"
            volume.mkdir()
            (volume / ".background.tiff").write_bytes(b"fixture")
            link = directory / "mount-link"
            link.symlink_to(volume, target_is_directory=True)
            with patch("dmg_layout.Alias.for_file") as alias:
                background_alias(link)
                alias.assert_called_once_with(str((volume / ".background.tiff").resolve()))
                self.assertEqual(alias.return_value.volume.posix_path, "/Volumes/Plotium")

    def test_dmg_layout_keeps_large_app_left_and_applications_right(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            (directory / ".background.tiff").write_bytes(b"fixture")
            date = datetime(2026, 1, 1, tzinfo=timezone.utc)
            alias = Alias(
                volume=VolumeInfo("Plotium", date, b"H+", 0, 0, b"\0\0",
                                  posix_path="/Volumes/Plotium"),
                target=TargetInfo(0, ".background.tiff", 2, 16, date, b"\0" * 4, b"\0" * 4,
                                  posix_path="/.background.tiff"),
            ).to_bytes()
            writer = patch("dmg_layout.background_alias", return_value=alias)
            writer.start()
            self.addCleanup(writer.stop)
            write_layout(directory)
            report = validate_layout(directory)
            self.assertEqual(report["icon_size"], 160)
            self.assertEqual(POSITIONS["Plotium.app"][1], POSITIONS["Applications"][1])
            self.assertLess(POSITIONS["Plotium.app"][0], POSITIONS["Applications"][0])
            with DSStore.open(str(directory / ".DS_Store"), "r+") as store:
                store["Plotium.app"]["Iloc"] = POSITIONS["Applications"]
            with self.assertRaisesRegex(ValueError, "position"):
                validate_layout(directory)
            write_layout(directory)
            with DSStore.open(str(directory / ".DS_Store"), "r+") as store:
                icons = store["."]["icvp"]
                icons["iconSize"] = 48.0
                store["."]["icvp"] = icons
            with self.assertRaisesRegex(ValueError, "large-icon"):
                validate_layout(directory)

    def test_mcp_smoke_uses_current_tools_and_scopes_receipt_queries_correctly(self):
        tools = [{"name": name, "inputSchema": {"required": [] if name in package_mcp.UNBOUND_TOOLS
                  else ["expected_project_context", "expected_revision"]}}
                 for name in sorted(package_mcp.EXPECTED_TOOLS)]
        package_mcp.validate_tools(tools)
        with self.assertRaises(AssertionError):
            package_mcp.validate_tools(tools[:-1])
        bound = next(t for t in tools if t["name"] == "plotium_plan")
        bound["inputSchema"]["required"].remove("expected_revision")
        with self.assertRaises(AssertionError):
            package_mcp.validate_tools(tools)

    def test_apple_version_mapping_is_numeric_distinct_and_ordered(self):
        versions = ["0.1.0-alpha.0", "0.1.0-alpha.1", "0.1.0-alpha.99999",
                    "0.1.0-beta.0", "0.1.0-beta.99999", "0.1.0", "0.1.1-alpha.0"]
        builds = [tuple(map(int, apple_versions(v)[1].split("."))) for v in versions]
        self.assertEqual(builds, sorted(set(builds)))
        self.assertEqual(apple_versions("0.1.0"), ("0.1.0", "0.1.200000"))
        self.assertEqual(apple_versions("0.1.0-beta.1"), ("0.1.0", "0.1.100001"))
        self.assertEqual(apple_versions("1.2.3-alpha.1"), ("1.2.3", "1.2.3000001"))

    def test_version_rejections_do_not_silently_truncate(self):
        for version in ["v0.1.0", "01.1.0", "0.1", "0.1.0-rc.1", "0.1.0+build.2"]:
            with self.subTest(version=version), self.assertRaises(ValueError):
                parse(version)
        for version in ["0.1.0-alpha.100000", "0.1.2148", "2147483648.0.0"]:
            with self.subTest(version=version), self.assertRaises(ValueError):
                apple_versions(version)
        # The portable release contract has no new Apple-only range constraint.
        self.assertIsNotNone(parse("0.1.0-alpha.100000"))

    def test_plist_registers_only_projects_and_keeps_full_cargo_version(self):
        info = info_plist("0.1.0-beta.1")
        self.assertEqual(info["CFBundleIdentifier"], "space.nmrtist.plotium")
        self.assertEqual(info["CFBundleExecutable"], "plotium")
        self.assertEqual(info["LSMinimumSystemVersion"], "13.0")
        self.assertEqual(info["PlotiumCargoVersion"], "0.1.0-beta.1")
        self.assertEqual(info["CFBundleDocumentTypes"][0]["CFBundleTypeRole"], "Editor")
        types = info["UTExportedTypeDeclarations"]
        self.assertEqual(types[0]["UTTypeTagSpecification"]["public.filename-extension"], ["plotium"])

    def test_binary_validation_checks_architecture_system_libraries_and_minos(self):
        with tempfile.TemporaryDirectory() as temporary:
            binary = Path(temporary) / "plotium"
            binary.write_bytes(b"fixture")
            binary.chmod(0o755)
            dependencies = "plotium:\n\t/usr/lib/libSystem.B.dylib (compatibility version 1.0.0)\n"
            with patch("subprocess.check_output", side_effect=["arm64", dependencies, "minos 13.0\n"]):
                self.assertEqual(inspect_binary(binary, "arm64")["minimum_macos"], "13.0")
            for outputs in [["x86_64"], ["arm64", "plotium:\n\t/opt/homebrew/lib/test.dylib (version 1)"],
                            ["arm64", dependencies, "minos 14.0\n"]]:
                with self.subTest(outputs=outputs), patch("subprocess.check_output", side_effect=outputs), self.assertRaises(ValueError):
                    inspect_binary(binary, "arm64")

    def test_collection_requires_two_zips_and_two_architecture_dmgs(self):
        self.assertEqual(expected_assets("0.2.0"), [
            "plotium-0.2.0-linux-x86_64.zip", "plotium-0.2.0-macos-aarch64.dmg",
            "plotium-0.2.0-macos-x86_64.dmg",
            "plotium-0.2.0-windows-x86_64.zip",
        ])
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            for name in expected_assets("0.1.0"):
                (directory / name).write_bytes(b"release asset")
            collect(directory, "0.1.0")
            sums = (directory / "SHA256SUMS").read_text().splitlines()
            self.assertEqual(len(sums), 4)
            self.assertEqual([line.split("  ")[1] for line in sums], expected_assets("0.1.0"))
            (directory / "plotium-0.1.0-macos-aarch64.dmg").unlink()
            with self.assertRaises(ValueError):
                collect(directory, "0.1.0")
            (directory / "plotium-0.1.0-macos-aarch64.dmg").write_bytes(b"")
            with self.assertRaises(ValueError):
                collect(directory, "0.1.0")

    def test_collection_rejects_extra_macos_zip_downloads(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            for name in expected_assets("0.2.0") + ["plotium-0.2.0-macos-aarch64.zip"]:
                (directory / name).write_bytes(b"package")
            with self.assertRaises(ValueError):
                collect(directory, "0.2.0")

    def test_flat_zip_retains_gui_connector_extension_and_licenses(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            target = "x86_64-pc-windows-msvc"
            binaries = root / "target" / target / "release"
            binaries.mkdir(parents=True)
            for name in ("plotium.exe", "plotium-mcp.exe"):
                (binaries / name).write_bytes(b"binary")
            for name in ("LICENSE", "COPYRIGHT.md", "README.md"):
                (root / name).write_text(name)

            def notice(*args, **kwargs):
                Path(args[-1]).write_text("license notice")

            def mcp(directory, platform, version, runtime_smoke=True):
                (directory / "plotium.mcpb").write_bytes(b"extension")

            with patch.object(package_portable, "ROOT", root), \
                    patch.object(package_portable, "cargo_version", return_value="0.1.0"), \
                    patch.object(package_portable, "run", side_effect=notice), \
                    patch.object(package_portable.package_mcp, "package", side_effect=mcp), \
                    patch.object(package_portable.package_mcp, "smoke") as smoke, \
                    patch.dict("os.environ", {"CARGO_TARGET_DIR": str(root / "target")}):
                package_portable.package(target, root / "dist", skip_build=True)
                smoke.assert_called_once()
            with zipfile.ZipFile(root / "dist/plotium-0.1.0-windows-x86_64.zip") as archive:
                self.assertEqual(set(archive.namelist()), {
                    "plotium.exe", "plotium-mcp.exe", "plotium.mcpb", "LICENSE", "COPYRIGHT.md",
                    "README.md", "THIRD-PARTY-LICENSES.html", "MCP-THIRD-PARTY-LICENSES.html",
                })


if __name__ == "__main__":
    unittest.main()
