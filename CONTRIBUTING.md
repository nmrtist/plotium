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

## Publish a release

The [Release workflow](.github/workflows/release.yml) packages the desktop app
and native MCP connector for Windows x86-64, Linux x86-64, and macOS
Intel / Apple Silicon. macOS and Windows archives also contain the native
Claude Desktop extension. The workflow exercises the connector's actual stdio
initialization and tool discovery before and after archive extraction.
It rechecks dependency policy before packaging, without rerunning the rest of
CI or requiring extra secrets.

1. Set `[workspace.package].version` in `Cargo.toml` and refresh `Cargo.lock`.
   Install `cargo-deny` if needed, then run `cargo release-check` locally. This
   runs `cargo pr-check` followed by `cargo dependency-check`. Resolve findings
   or document justified exceptions before submitting the version PR. Merge
   the change and wait for CI to pass on the release commit.
2. Tag that commit with the Cargo version prefixed by `v`, then push the tag:

   ```sh
   git tag -a v0.1.0 -m "Plotium 0.1.0"
   git push origin v0.1.0
   ```

3. Download the packages from the generated Release draft, check the desktop
   app, MCP connector, and license notices on each platform, then edit the notes and publish.

The release dependency check is a final safeguard: a new advisory published
after local or PR checks can still stop packaging. Reproduce with
`cargo dependency-check`, address the finding, and prepare a new release commit
and tag rather than moving an existing tag.

Use `v0.1.0` for a regular pre-1.0 release. Only `-alpha.N` and `-beta.N`
suffixes are supported for prereleases; they also select the updater channel.

The draft includes unsigned portable ZIPs with license notices and `SHA256SUMS`.
Platform requirements are in the
[installation guide](https://plotium.nmrtist.space/getting-started/installation/).

For a rehearsal, use **Actions → Release → Run workflow** on the chosen ref;
this uploads Actions artifacts without creating a release. Failed runs can be
retried to finish a draft; published releases require a new version.

## Contribution licensing

Contributions use the project's `GPL-3.0-or-later` license; contributors retain
their copyright. Make sure you have the right to contribute, including any
employer rights. Document the source and license of third-party material and
data files. Check new dependency licenses yourself: CI does not check license
compatibility. See [COPYRIGHT.md](COPYRIGHT.md) for the licensing notice.
