# Contributing to Plotium

## Start developing

Use the current stable Rust toolchain with `rustfmt` and `clippy`. Run these
commands from the repository root:

```sh
rustup component add rustfmt clippy
cargo run -p plotium
```

For MCP integration work, first run `cargo build -p plotium-agent --bin plotium-mcp`.
The desktop app installs this sibling executable into its stable user data path
when MCP is enabled. Protocol and connector code live in `crates/agent/`.

For the edit/check/test loop, target the frontend or crate you changed:

| Purpose | Command |
| --- | --- |
| Check the desktop app | `cargo check -p plotium` |
| Run the CLI | `cargo run -p plotium-cli -- --help` |
| Test a changed crate (example) | `cargo test -p plotium-core` |
| Build the shipping app / assess performance | `cargo release-build` |
| Check dependency policy | `cargo dependency-check` |
| Check a release locally | `cargo release-check` |

Development builds optimize numerical processing crates, but leave the UI and
renderer unoptimized. Use a release build to assess rendering performance.

Default builds use the reference table executor. Avoid `--workspace` during
routine development: it also selects the optional DataFusion and Substrait
crates. For explicit backend maintenance, use `cargo datafusion-build`;
`cargo pr-check` and CI do not validate those crates.

## Find the code

| Area | Location |
| --- | --- |
| Desktop UI | [`crates/app/`](crates/app/) — read its [UI rules](crates/app/AGENTS.md) before editing |
| CLI | [`crates/cli/`](crates/cli/) |
| Application state, actions, and workflows | [`crates/core/`](crates/core/) |
| Data model and table semantics | [`crates/data/`](crates/data/) |
| File parsing and import fixtures | [`crates/io/`](crates/io/) |
| Scientific algorithms and spectral transforms | [`crates/analysis/`](crates/analysis/), [`crates/processing/`](crates/processing/) |
| Figure models and rendering/export | [`crates/figure/`](crates/figure/), [`crates/render/`](crates/render/) |

[AGENTS.md](AGENTS.md) records repository-wide invariants, including typed IDs,
crate boundaries, and persisted-format rules.

For UI screenshots, set `PLOTIUM_SHOT` to an output directory and run the app.
The harness runs a synthetic session, captures both themes, and exits; see
[`shot.rs`](crates/app/src/shot.rs) for scene and theme controls.

## Edit the manual

The Astro/Starlight manual lives in `docs/`. With Node.js LTS installed:

```sh
cd docs
npm ci
npm run dev
```

English pages in `docs/src/content/docs/` are canonical. Keep matching pages
under `zh-cn/` aligned when changing translated content. Run `npm run build`
from `docs/` before submitting. See [docs/README.md](docs/README.md) for the site
layout and deployment details.

## Before a pull request

- Add or update tests for changed behavior, especially numerical results and
  import edge cases. For UI changes, also check the affected interaction in the app.
- Run `cargo pr-check` for Rust changes. It checks formatting, the 800-line Rust
  source limit, both frontends, Clippy, and reference-backend tests.
  To rerun a failed stage, use `cargo pr-check quick`, `cargo pr-check lint`, or
  `cargo pr-check test`.
- For dependency or dependency-policy changes, install `cargo-deny` with
  `cargo install --locked cargo-deny` and run `cargo dependency-check`.
  It checks advisories, duplicate versions, and dependency sources for the
  reference backend on all shipping platforms; it does not check licenses.
  CI runs the same command when Cargo manifests, `Cargo.lock`, `deny.toml`,
  Cargo aliases, xtask, or workflows change. Ordinary PRs skip this check.
  Duplicate versions are warnings; unmaintained transitive dependencies do not
  block the check. Resolve findings or record a justified exception in
  `deny.toml`, including the reason and when to review it (for example, on a
  relevant dependency upgrade).
- Update the manual for user-visible changes and build it as described above.
- Explain the problem, the resulting behavior, and how you validated the change
  in the pull request. Include screenshots for visual changes.
- Keep generated output (`target/`, `dist/`, `docs/dist/`, `docs/node_modules/`)
  out of commits. Use minimal anonymized samples; never include confidential,
  personal, or proprietary data in code, issues, or pull requests.

The [Dependency check workflow](.github/workflows/dependency-check.yml) can also
be run manually in Actions. It has no scheduled runs. Findings appear in the
job logs, with a status summary on the workflow run.

## Package a macOS application locally

Build a DMG that can be dragged into Applications. On macOS with Python 3.10+,
Apple command-line tools, and the matching Rust target:

```sh
cargo install cargo-about --version 0.9.1 --locked --features cli
python3 -m venv /tmp/plotium-packaging-python
/tmp/plotium-packaging-python/bin/python -m pip install -r .github/scripts/requirements-packaging.txt
/tmp/plotium-packaging-python/bin/python .github/scripts/package_macos.py --target aarch64-apple-darwin
# On an Intel Mac, use --target x86_64-apple-darwin instead.
```

The script builds the default reference-executor GUI and MCP connector with
`MACOSX_DEPLOYMENT_TARGET=13.0`, collects license notices, renders the existing SVG
brand assets at each ICNS resolution, signs locally with an ad-hoc identity, and
creates `dist/plotium-<version>-macos-<architecture>.dmg`. Finder opens a compact
icon-view window with 160-point icons: Plotium on the left, Applications on the
right, with a right-pointing arrow between them. The arrow is a Retina-aware
background, not a third draggable item. The layout and its on-volume background
reference are written and checked without running Finder on CI.
The script also mounts the final
DMG read-only, verifies its Applications shortcut, copies the app to
`dist/installed-macos-<architecture>/Plotium.app`, and validates that installed
copy. It never installs into Applications or launches the GUI automatically.
Quit any running test copy before rebuilding packages into the same output folder.
Use `--skip-build` only with matching release binaries already built for macOS 13;
CI calls the same implementation with that option. `CARGO_TARGET_DIR` is respected.
For cross-packaging on the other macOS architecture, install that Rust target
and pass `--skip-connector-smoke`. This preserves all static/package checks but
explicitly records that the connector was not executed; CI on the matching
architecture never uses this flag. It does not establish GUI runtime acceptance.

`dist/macos-validation-macos-<architecture>.json` records the actual Mach-O
architecture, deployment target, dependencies, signature, quarantine attribute,
and Gatekeeper assessment. A rejected Gatekeeper assessment is recorded, not
reported as acceptance. These DMGs are **ad-hoc signed, not Developer ID signed or
notarized**. Local mounting and launching do not establish Internet-download
acceptance. No packaging step removes quarantine or changes Gatekeeper policy.

For interactive acceptance, copy from the mounted DMG to a separate test folder,
eject it, then launch that copy through Finder. Check project open/edit/save/reopen,
Finder project opens both cold and while running, unsaved-change confirmation,
window close/Dock reopen/Quit, and a damaged project error. Confirm the MCP
connector and Claude Desktop extension still resolve inside the app. Do not
overwrite an existing installation during development. Record the tested OS and
architecture; other architectures need their own runtime acceptance.

Cargo metadata is the only version source. `CFBundleShortVersionString` is
`X.Y.Z`; `PlotiumCargoVersion` keeps the complete SemVer. `CFBundleVersion` is
`X.Y.(Z * 1000000 + stage + N)`, with stage 0 for alpha, 100000 for beta, and
200000 for stable (N = 0 for stable). For example, `0.1.0-beta.1` becomes
`0.1.100001`, and `0.1.0` becomes `0.1.200000`. Prerelease N must be 0–99999;
Apple version components must fit signed 32-bit integers. Invalid values fail
instead of being truncated. This deterministic mapping needs no separate counter.
The application About panel continues to show the Cargo version.

Test packaging contracts and vector rendering with:

```sh
/tmp/plotium-packaging-python/bin/python -B -m unittest discover -s .github/scripts -p 'test_*.py'
cargo test --locked -p xtask --features macos-packaging
```

The pinned Python tools only write and inspect Finder layout metadata during
packaging/tests; they are not application runtime dependencies. The optional
xtask feature reuses the workspace SVG renderer.

The current no-paid-account distribution uses ad-hoc signatures and explicitly
advertises the lack of notarization. Test browser downloads on real Macs and
record any first-launch approval needed under Privacy & Security; do not disable
Gatekeeper or remove quarantine attributes. This is not equivalent to Developer ID
publisher authentication or notarization. If a paid developer membership becomes
available, add Developer ID signing, hardened runtime, notarization, and stapling;
configure credentials and call Apple's service only with separate authorization.

From 0.2.0, application bundles update from the matching processor's DMG by
replacing the complete app after exit, with rollback on replacement failure.
The Release workflow tests its actual DMG and shipping helper on both macOS
architectures. This does not replace browser-download and GUI launch acceptance.

## Publish a release

The [Release workflow](.github/workflows/release.yml) packages the desktop app
and native MCP connector for Windows x86-64, Linux x86-64, and macOS
Intel / Apple Silicon. macOS applications and Windows ZIPs also contain the native
Claude Desktop extension. The workflow exercises the connector's actual stdio
initialization and tool discovery in packaged installations. The macOS jobs
also exercise their actual DMGs through the shipping update helper.
It rechecks dependency policy before packaging, without rerunning the rest of
CI or requiring extra secrets.

1. Set `[workspace.package].version` in `Cargo.toml` and refresh `Cargo.lock`.
   Install `cargo-deny` if needed, then run `cargo release-check` locally. This
   runs `cargo pr-check` followed by `cargo dependency-check`. Resolve findings
   or document justified exceptions before submitting the version PR. Merge
   the change and wait for CI to pass on the release commit.
2. Tag that commit with the Cargo version prefixed by `v`, then push the tag.
   Replace `X.Y.Z` below with the complete Cargo version, including any
   prerelease suffix:

   ```sh
   git tag -a vX.Y.Z -m "Plotium X.Y.Z"
   git push origin vX.Y.Z
   ```

3. Download the packages from the generated Release draft, check the desktop
   app, MCP connector, and license notices on each platform, then edit the notes and publish.

The release dependency check is a final safeguard: a new advisory published
after local or PR checks can still stop packaging. Reproduce with
`cargo dependency-check`, address the finding, and prepare a new release commit
and tag rather than moving an existing tag.

Only `-alpha.N` and `-beta.N` suffixes are supported for prereleases; they also
select the updater channel.

The draft contains exactly four install packages: Windows and Linux ZIPs, and
Intel and Apple Silicon macOS DMGs. License notices are inside the packages;
`SHA256SUMS` covers all four. Do not publish macOS ZIPs or a universal DMG.
macOS apps are ad-hoc signed and unnotarized.
Platform requirements are in the
[installation guide](https://plotium.nmrtist.space/getting-started/installation/).

For a rehearsal, use **Actions → Release → Run workflow** on the chosen ref;
this builds and validates the same four-package asset set and uploads it as
`validated-release`, without creating a release. Per-architecture macOS validation
reports are separate Actions artifacts. Failed runs can be
retried to finish a draft; published releases require a new version.

## Contribution licensing

Contributions use the project's `GPL-3.0-or-later` license; contributors retain
their copyright. Make sure you have the right to contribute, including any
employer rights. Document the source and license of third-party material and
data files. Check new dependency licenses yourself: CI does not check license
compatibility. See [COPYRIGHT.md](COPYRIGHT.md) for the licensing notice.
