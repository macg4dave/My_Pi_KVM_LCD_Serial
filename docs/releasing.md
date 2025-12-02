# Local packaging and GitHub releases

Everything here is meant to be run locally (no CI). The packaging metadata lives in `Cargo.toml`
and installs both the binary and the `lifelinetty.service` unit file.

## Prerequisites

- Rust toolchain with `cargo`
- Packaging helpers: `cargo install cargo-deb cargo-generate-rpm`
- Docker with BuildKit/buildx (used for armv6/armv7/arm64 cross builds)
- System tools: `dpkg-dev` (for Debian/Ubuntu), `rpm-build` (for Fedora/RHEL), `systemd`
- Optional: GitHub CLI (`gh`) with `GH_TOKEN` configured for uploads

## Manual build and package (host architecture)

```sh
# 1) Ensure Cargo.toml version is set for the release
cargo build --release

# 2) Build Debian and RPM packages using the existing artifacts
cargo deb --no-build
cargo generate-rpm

# Artifacts
# - target/release/lifelinetty
# - target/debian/lifelinetty_<version>_<arch>.deb
# - target/generate-rpm/lifelinetty-<version>-1.<arch>.rpm
```

To build for a specific target (e.g. Raspberry Pi armv7), add `--target <triple>` to the three
commands above, and ensure the appropriate cross toolchain is installed.

## Helper script for repeatable local releases

`scripts/local-release.sh` wraps the steps above, copies artifacts into `releases/<version>/`,
renames them with a predictable `lifelinetty_v<version>_<arch>` pattern, and can optionally publish
them to GitHub Releases.

Examples:

```sh
# Host architecture, build + package only
scripts/local-release.sh

# Cross build for armv7 and upload to GitHub (requires existing git tag v0.1.0)
scripts/local-release.sh --target armv7-unknown-linux-gnueabihf --upload
```

Script flags:
- `--target <triple>`: cross-compile/pack for another target (can be repeated)
- `--targets <t1,t2>`: comma-separated list of targets
- `--all-targets`: build host + armv6 + armv7 + arm64. The predefined set uses Docker Buildx for the ARM targets, so you don't need local cross toolchains.
- `--tag <git-tag>`: override the release tag (default: `v<Cargo version>`)
- `--upload`: create/update the GitHub release with the generated artifacts
- `--all`: convenience alias for `--upload` (build + package + upload in one go)

Outputs in `releases/<version>/` are named like:
 - `lifelinetty_v0.5_armv6` (raw binary)
 - `lifelinetty_v0.5_armv6.deb`
 - `lifelinetty_v0.5_armv6.rpm`

Note: Cross targets must be installed and have working linkers. For example:

```sh
# Only needed if you build cross-arch outside Docker
rustup target add arm-unknown-linux-musleabihf armv7-unknown-linux-gnueabihf aarch64-unknown-linux-gnu
```

## Publishing a GitHub release manually

```sh
git tag v0.1.0        # tag should match Cargo.toml version
scripts/local-release.sh            # builds packages into releases/0.1.0/
gh release create v0.1.0 releases/0.1.0/* \
  --title "lifelinetty v0.1.0" \
  --notes "Local release of lifelinetty v0.1.0"
```

## Installing the packages

```sh
# Debian/Ubuntu
sudo dpkg -i lifelinetty_0.1.0_armhf.deb
sudo systemctl enable --now lifelinetty.service

# Fedora/RHEL
sudo rpm -Uvh lifelinetty-0.1.0-1.armv7hl.rpm
sudo systemctl enable --now lifelinetty.service
```

The post-install scripts only reload the systemd unit cache; enabling/starting the service is
left to the operator so you can adjust config and wiring first.

## Migration notes: SerialLCD -> LifelineTTY

If you're upgrading from the previous SerialLCD releases, here's what changed and how to migrate smoothly:

- Binary rename: the runtime binary and packaged artifacts are now called `lifelinetty`. Packaging will create a compatibility copy at `/usr/bin/seriallcd` for backwards compatibility, but it's recommended to update scripts to call `lifelinetty` directly.
- Systemd unit: the primary unit file is `lifelinetty.service`. To ease transition, installer scripts will create a compatibility symlink called `seriallcd.service` that points to `lifelinetty.service`. You can safely enable either service, but use `lifelinetty.service` going forward.
- Config path: no changes — the configuration file remains at `~/.serial_lcd/config.toml` to avoid surprises. Existing config will be used by the new binary.
- CLI & env var compatibility: CLI flags are unchanged; environment variables for logging now prefer `LIFELINETTY_LOG_*`, but the legacy `SERIALLCD_LOG_*` variables continue to be accepted for compatibility.
- Release artifacts: the `releases/` artifacts use `lifelinetty_v<version>_<arch>` names. Old `seriallcd_*` artifacts may exist in historical releases.

Suggested steps after installing the new package:

1. Verify the new unit and binary exist: `which lifelinetty` and `systemctl status lifelinetty.service`.
2. If you have scripts, crons, or tooling that call `seriallcd`, either update them to `lifelinetty` or keep the compatibility symlink until you can migrate.
3. If you previously enabled `seriallcd.service`, check whether the compatibility symlink was created; otherwise `systemctl enable --now lifelinetty.service`.

If you run into issues, see `docs/architecture.md` and the logs (`LIFELINETTY_LOG_PATH` or `SERIALLCD_LOG_PATH`) for troubleshooting.
