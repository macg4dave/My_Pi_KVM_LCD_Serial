# Docker cross-build for Raspberry Pi 1 (ARMv6)

This setup builds an `arm-unknown-linux-musleabihf` binary (static musl) and a runnable image for BCM2835-based Pis (Raspberry Pi 1 / Zero).

## Requirements

- Docker with BuildKit + `buildx` enabled.
- Internet access to pull base images and toolchains.

## Build the image

```sh
docker buildx build \
  --platform linux/arm/v6 \
  -f docker/Dockerfile.armv6 \
  -t lifelinetty:armv6 \
  .
```

The build uses a multi-stage pipeline:

- Builder: `rust:<version>-bookworm` plus the prebuilt `armv6-linux-musleabihf` toolchain from musl.cc. Target is Armv6 + VFP2 and the output is fully static.
- Runtime: `scratch` with only the compiled binary copied in.

Defaults:

- Target: `arm-unknown-linux-musleabihf`
- CPU tuning: `-C target-cpu=arm1176jzf-s -C target-feature=+vfp2` for BCM2835 (Pi 1 / Zero)
 - Entry: `lifelinetty --run`

## Debug build (optional)

```sh
docker buildx build \
  --platform linux/arm/v6 \
  -f docker/Dockerfile.armv6 \
  --build-arg RUSTFLAGS="-C target-cpu=arm1176jzf-s -C target-feature=+vfp2 -C debuginfo=2" \
  -t lifelinetty:armv6-debug \
  .
```

## Running the image (on a Pi)

```sh
docker run --rm --name lifelinetty \
  --device /dev/ttyUSB0 \
  --device /dev/i2c-1 \
  lifelinetty:armv6 --run
```

## Notes

- Output is statically linked via musl, so no extra runtime libraries are required and it avoids the Debian armhf (ARMv7) baseline that can raise `Illegal instruction` on Pi 1/Zero.
- If you need async serial, build with `--build-arg RUSTFLAGS=...` and enable the feature: `cargo build --features async-serial ...` (adjust the Dockerfile command as needed).
- The Dockerfile uses cache mounts for cargo registry/git/target to speed up iterative builds.
- Keep `/run/serial_lcd_cache` as the only writable path inside the container (bind-mount if needed). If your adapter lives on another TTY (`/dev/ttyAMA0`, `/dev/ttyS0`, USB serial numbers, etc.), either change the `--device` mapping above or set `device = "..."` in `~/.serial_lcd/config.toml` inside the container volume.
