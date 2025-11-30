BIN := seriallcd
ARMV6_TARGET := arm-unknown-linux-musleabihf
ARMV7_TARGET := armv7-unknown-linux-gnueabihf
AARCH64_TARGET := aarch64-unknown-linux-gnu
OUT_ROOT := releases/debug
X86_DIR := $(OUT_ROOT)/x86
ARMV6_DIR := $(OUT_ROOT)/armv6
ARMV7_DIR := $(OUT_ROOT)/armv7
AARCH64_DIR := $(OUT_ROOT)/arm64
EXT := $(if $(filter Windows_NT,$(OS)),.exe,)
ARMV6_DOCKER_IMAGE := seriallcd:armv6
ARMV6_DOCKERFILE := docker/Dockerfile.armv6
ARMV7_DOCKER_IMAGE := seriallcd:armv7
ARMV7_DOCKERFILE := docker/Dockerfile.armv7
ARM64_DOCKER_IMAGE := seriallcd:arm64
ARM64_DOCKERFILE := docker/Dockerfile.arm64

.PHONY: all x86 armv6 armv7 arm64 test clean

# Build all targets: native + Raspberry Pi variants.
all: x86 armv6 armv7 arm64

# Native x86_64 release (uses host default target).
x86:
	cargo build --release
	mkdir -p $(X86_DIR)
	cp target/release/$(BIN)$(EXT) $(X86_DIR)/

test:
	cargo test

# ARMv6 (Pi 1/Zero) release via Docker cross-build.
armv6:
	mkdir -p $(ARMV6_DIR)
	docker buildx build --platform linux/arm/v6 --load \
		-f $(ARMV6_DOCKERFILE) \
		-t $(ARMV6_DOCKER_IMAGE) \
		.
	@cid=$$(docker create $(ARMV6_DOCKER_IMAGE)); \
	docker cp $$cid:/usr/local/bin/$(BIN) $(ARMV6_DIR)/; \
	docker rm $$cid >/dev/null

# ARMv7 (Pi 2/3 32-bit) native cross-build (requires Rust targets/toolchains installed).
armv7:
	mkdir -p $(ARMV7_DIR)
	docker buildx build --platform linux/arm/v7 --load \
		-f $(ARMV7_DOCKERFILE) \
		-t $(ARMV7_DOCKER_IMAGE) \
		.
	@cid=$$(docker create $(ARMV7_DOCKER_IMAGE)); \
	docker cp $$cid:/usr/local/bin/$(BIN) $(ARMV7_DIR)/; \
	docker rm $$cid >/dev/null

# ARM64 (Pi 3/4/5 64-bit) native cross-build.
arm64:
	mkdir -p $(AARCH64_DIR)
	docker buildx build --platform linux/arm64/v8 --load \
		-f $(ARM64_DOCKERFILE) \
		-t $(ARM64_DOCKER_IMAGE) \
		.
	@cid=$$(docker create $(ARM64_DOCKER_IMAGE)); \
	docker cp $$cid:/usr/local/bin/$(BIN) $(AARCH64_DIR)/; \
	docker rm $$cid >/dev/null

# Remove build outputs.
clean:
	rm -rf $(OUT_ROOT)
