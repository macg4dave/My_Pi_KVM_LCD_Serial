# Local packaging and GitHub releases

Everything here is meant to be run locally (no CI). The packaging metadata lives in `Cargo.toml`
and installs both the binary and the `seriallcd.service` unit file.

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
# - target/release/seriallcd
# - target/debian/seriallcd_<version>_<arch>.deb
# - target/generate-rpm/seriallcd-<version>-1.<arch>.rpm
```

To build for a specific target (e.g. Raspberry Pi armv7), add `--target <triple>` to the three
commands above, and ensure the appropriate cross toolchain is installed.

## Helper script for repeatable local releases

`scripts/local-release.sh` wraps the steps above, copies artifacts into `releases/<version>/`,
renames them with a predictable `seriallcd_v<version>_<arch>` pattern, and can optionally publish
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
- `seriallcd_v0.5_armv6` (raw binary)
- `seriallcd_v0.5_armv6.deb`
- `seriallcd_v0.5_armv6.rpm`

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
  --title "seriallcd v0.1.0" \
  --notes "Local release of seriallcd v0.1.0"
```

## Installing the packages

```sh
# Debian/Ubuntu
sudo dpkg -i seriallcd_0.1.0_armhf.deb
sudo systemctl enable --now seriallcd.service

# Fedora/RHEL
sudo rpm -Uvh seriallcd-0.1.0-1.armv7hl.rpm
sudo systemctl enable --now seriallcd.service
```

The post-install scripts only reload the systemd unit cache; enabling/starting the service is
left to the operator so you can adjust config and wiring first.
