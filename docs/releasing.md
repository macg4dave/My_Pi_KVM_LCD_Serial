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
  
  Note: the helper script now prefers a native/host build when possible. For example, on an aarch64 host the script will attempt a native cargo build for the aarch64 target when the corresponding rustup target is installed. This avoids running Docker when you're already on the correct hardware. The script still falls back to Docker when running inside a container, when the rustup target is missing, or when `FORCE_DOCKER=1` is set.

  Use `USE_HOST_BUILD=0` to disable host/native builds (force cargo to use Docker path where applicable), or `FORCE_DOCKER=1` to always use Docker. These environment variables are honored by both `scripts/local-release.sh` and the top-level `Makefile`.
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

SerialLCD was an alpha preview and is no longer supported. No backward compatibility is maintained.

If you were using SerialLCD, upgrade to LifelineTTY:

- **Binary**: download the latest `lifelinetty_v*_<arch>` release binary from the GitHub releases page.
- **Systemd unit**: enable `lifelinetty.service` (the previous `seriallcd.service` is not provided).
- **Config path**: configuration remains at `~/.serial_lcd/config.toml`; your existing configs are compatible with the new binary.
- **CLI & env vars**: update any scripts that call `seriallcd` to use `lifelinetty`. Use `LIFELINETTY_LOG_*` environment variables for logging.
- **Release artifacts**: all new artifacts use the `lifelinetty_v<version>_<arch>` naming scheme.
- **Compatibility note**: installer scripts no longer create `/usr/bin/seriallcd` or `seriallcd.service` symlinks—remove any manual leftovers when upgrading.

Steps to migrate:

1. Stop the old service: `sudo systemctl stop seriallcd.service` (if running).
2. Install LifelineTTY: `sudo apt install ./lifelinetty_*.deb` or equivalent for your platform.
3. Update any scripts or crons that invoke `seriallcd` to use `lifelinetty`.
4. Enable and start the new service: `sudo systemctl enable --now lifelinetty.service`.
5. Verify: `which lifelinetty` and `systemctl status lifelinetty.service`.

For troubleshooting, see `docs/architecture.md` and logs via `LIFELINETTY_LOG_PATH`.
