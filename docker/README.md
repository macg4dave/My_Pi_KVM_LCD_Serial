# Docker cross-build for Raspberry Pi 1 (ARMv6)

This setup builds an `arm-unknown-linux-gnueabihf` binary and a runnable image for BCM2835-based Pis (Raspberry Pi 1 / Zero).

## Requirements
- Docker with BuildKit + `buildx` enabled.
- Internet access to pull base images and toolchains.

## Build the image
```sh
docker buildx build \
  --platform linux/arm/v6 \
  -f docker/Dockerfile.armv6 \
  -t seriallcd:armv6 \
  .
```

The build uses a multi-stage pipeline:
- Builder: `rust:<version>-bookworm` with the ARMv6 cross toolchain (`gcc-arm-linux-gnueabihf`, `libc6-dev-armhf-cross`).
- Runtime: `debian:bookworm-slim` for `linux/arm/v6`, with runtime libs only.

Defaults:
- Target: `arm-unknown-linux-gnueabihf`
- CPU tuning: `-C target-cpu=arm1176jzf-s -C target-feature=+vfp2` for BCM2835
- Entry: `seriallcd --run`

## Debug build (optional)
```sh
docker buildx build \
  --platform linux/arm/v6 \
  -f docker/Dockerfile.armv6 \
  --build-arg RUSTFLAGS="-C target-cpu=arm1176jzf-s -C target-feature=+vfp2 -C debuginfo=2" \
  -t seriallcd:armv6-debug \
  .
```

## Running the image (on a Pi)
```sh
docker run --rm --name seriallcd \
  --device /dev/ttyAMA0 \
  --device /dev/i2c-1 \
  seriallcd:armv6 --run
```

## Notes
- If you need async serial, build with `--build-arg RUSTFLAGS=...` and enable the feature: `cargo build --features async-serial ...` (adjust the Dockerfile command as needed).
- The Dockerfile uses cache mounts for cargo registry/git/target to speed up iterative builds.
- Keep `/run/serial_lcd_cache` as the only writable path inside the container (bind-mount if needed).***
