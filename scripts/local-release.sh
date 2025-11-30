#!/usr/bin/env bash
set -euo pipefail

usage() {
    cat <<'EOF'
Build release artifacts locally (binary + .deb + .rpm) and optionally upload them to
GitHub Releases. The version is read from Cargo.toml; tag defaults to "v<version>".

Usage: scripts/local-release.sh [--target <triple>] [--targets <t1,t2>] [--all-targets] [--tag <git-tag>] [--upload|--all]

  --target <triple>   Optional Rust target triple (can be repeated). Example:
                      armv7-unknown-linux-gnueabihf
  --targets <list>    Comma-separated list of target triples (overrides --target)
  --all-targets       Build for host + armv6 + armv7 + arm64 (predefined list)
  --tag <git-tag>     Override release tag (default: v<Cargo version>)
  --upload            Push artifacts to GitHub Releases using the GitHub CLI.
  --all               Convenience: build + package + upload (same as passing --upload).
  -h, --help          Show this message.

Prereqs: cargo, cargo-deb, cargo-generate-rpm, python3, and optionally the gh CLI.
For cross builds via Docker, you also need Docker BuildKit/buildx.
EOF
}

require_cmd() {
    if ! command -v "$1" >/dev/null 2>&1; then
        echo "Missing required tool: $1" >&2
        exit 1
    fi
}

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGETS=()
TAG_OVERRIDE=""
UPLOAD=0
ALL_TARGETS=0
UPLOAD_ASSETS=()
ALL_TARGETS_DEFAULT=("" "arm-unknown-linux-musleabihf" "armv7-unknown-linux-gnueabihf" "aarch64-unknown-linux-gnu")

derive_arch_label() {
    local triple="${1:-}"
    case "${triple}" in
        arm-unknown-linux-musleabihf) echo "armv6"; return ;;
        armv7-unknown-linux-gnueabihf) echo "armv7"; return ;;
        aarch64-unknown-linux-gnu) echo "arm64"; return ;;
    esac
    if [[ -n "${triple}" ]]; then
        case "${triple}" in
            *armv6*) echo "armv6" ;;
            *armv7*) echo "armv7" ;;
            *aarch64*|*arm64*) echo "arm64" ;;
            *x86_64*|*amd64*) echo "x86_64" ;;
            *i686*|*i386*) echo "x86" ;;
            *) echo "${triple}" ;;
        esac
        return
    fi

    local uname_arch
    uname_arch="$(uname -m)"
    case "${uname_arch}" in
        armv6*) echo "armv6" ;;
        armv7*) echo "armv7" ;;
        aarch64|arm64) echo "arm64" ;;
        x86_64|amd64) echo "x86_64" ;;
        i686|i386) echo "x86" ;;
        *) echo "${uname_arch}" ;;
    esac
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --target)
            TARGETS+=("${2:-}")
            shift 2
            ;;
        --targets)
            IFS=',' read -r -a TARGETS <<< "${2:-}"
            shift 2
            ;;
        --all-targets)
            ALL_TARGETS=1
            shift
            ;;
        --tag)
            TAG_OVERRIDE="${2:-}"
            shift 2
            ;;
        --all)
            UPLOAD=1
            shift
            ;;
        --upload)
            UPLOAD=1
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "Unknown argument: $1" >&2
            usage
            exit 1
            ;;
    esac
done

require_cmd cargo
require_cmd cargo-deb
require_cmd cargo-generate-rpm
require_cmd python3

CRATE_VERSION="$(
    cargo metadata --no-deps --format-version 1 |
        python3 -c '
import json, sys
meta = json.load(sys.stdin)
for pkg in meta.get("packages", []):
    if pkg.get("name") == "seriallcd":
        print(pkg.get("version"))
        sys.exit(0)
print("Failed to find seriallcd in cargo metadata", file=sys.stderr)
sys.exit(1)
'
)"

if [[ "${ALL_TARGETS}" -eq 1 ]]; then
    TARGETS=("${ALL_TARGETS_DEFAULT[@]}")
fi

if [[ "${#TARGETS[@]}" -eq 0 ]]; then
    TARGETS=("")
fi

if [[ -z "${CRATE_VERSION}" ]]; then
    echo "Could not determine crate version" >&2
    exit 1
fi

RELEASE_TAG="${TAG_OVERRIDE:-v${CRATE_VERSION}}"

package_artifacts() {
    local triple="$1"
    local arch_label="$2"
    local target_dir="$3"
    local deb_args_str="$4"
    local rpm_args_str="$5"

    local BIN_PATH="${ROOT}/${target_dir}/release/seriallcd"
    local DEB_DIR="${ROOT}/${target_dir}/debian"
    local RPM_DIR="${ROOT}/${target_dir}/generate-rpm"

    IFS=' ' read -r -a deb_args <<< "${deb_args_str}"
    IFS=' ' read -r -a rpm_args <<< "${rpm_args_str}"

    local env_prefix=()
    if [[ -n "${triple}" ]]; then
        local env_var
        env_var=$(echo "CARGO_TARGET_${triple}_STRIP" | tr '[:lower:]-' '[:upper:]_')
        env_prefix+=("${env_var}=/bin/true")
    fi

    local strip_shim
    strip_shim="$(mktemp -d)"
    cat > "${strip_shim}/strip" <<'EOS'
#!/bin/sh
# Minimal strip shim that just copies input to requested output.
out=""
in=""
while [ $# -gt 0 ]; do
  case "$1" in
    -o)
      out="$2"
      shift 2
      ;;
    *)
      in="$1"
      shift
      ;;
  esac
done

if [ -n "$out" ] && [ -n "$in" ] && [ -f "$in" ]; then
  cp "$in" "$out"
  exit 0
fi

# Fallback: if no -o provided, just succeed.
exit 0
EOS
    chmod +x "${strip_shim}/strip"
    local shimmed_path="${strip_shim}:$PATH"

    if [[ ! -f "${BIN_PATH}" ]]; then
        echo "Binary not found at ${BIN_PATH}" >&2
        exit 1
    fi

    env ${env_prefix[@]+"${env_prefix[@]}"} PATH="${shimmed_path}" CARGO_PROFILE_RELEASE_STRIP=false cargo deb "${deb_args[@]}"
    env ${env_prefix[@]+"${env_prefix[@]}"} PATH="${shimmed_path}" CARGO_PROFILE_RELEASE_STRIP=false cargo generate-rpm "${rpm_args[@]}"

    local DEB_PATH
    local RPM_PATH
    DEB_PATH="$(ls -t "${DEB_DIR}"/seriallcd_*.deb 2>/dev/null | head -n 1 || true)"
    RPM_PATH="$(ls -t "${RPM_DIR}"/seriallcd-*.rpm 2>/dev/null | head -n 1 || true)"

    if [[ -z "${DEB_PATH}" ]]; then
        echo "No .deb artifact found in ${DEB_DIR}" >&2
        exit 1
    fi

    if [[ -z "${RPM_PATH}" ]]; then
        echo "No .rpm artifact found in ${RPM_DIR}" >&2
        exit 1
    fi

    local BIN_OUT="${OUT_DIR}/seriallcd_v${CRATE_VERSION}_${arch_label}"
    local DEB_OUT="${OUT_DIR}/seriallcd_v${CRATE_VERSION}_${arch_label}.deb"
    local RPM_OUT="${OUT_DIR}/seriallcd_v${CRATE_VERSION}_${arch_label}.rpm"

    cp "${BIN_PATH}" "${BIN_OUT}"
    cp "${DEB_PATH}" "${DEB_OUT}"
    cp "${RPM_PATH}" "${RPM_OUT}"

    echo "Artifacts written to ${OUT_DIR}:"
    echo "  $(basename "${BIN_OUT}")"
    echo "  $(basename "${DEB_OUT}")"
    echo "  $(basename "${RPM_OUT}")"

    UPLOAD_ASSETS+=("${BIN_OUT}" "${DEB_OUT}" "${RPM_OUT}")
}

build_with_cargo() {
    local triple="$1"
    local arch_label="$2"

    local target_dir="target"
    local deb_args=(--no-build)
    local rpm_args=()
    local build_args=(--release)

    if [[ -n "${triple}" ]]; then
        if ! rustup target list --installed | grep -qx "${triple}"; then
            echo "Rust target ${triple} not installed. Install with: rustup target add ${triple}" >&2
            exit 1
        fi
        build_args+=(--target "${triple}")
        deb_args+=(--target "${triple}")
        rpm_args+=(--target "${triple}")
        target_dir="target/${triple}"
    fi

    echo "Building seriallcd ${CRATE_VERSION} (${arch_label})..."
    cargo build "${build_args[@]}"
    package_artifacts "${triple}" "${arch_label}" "${target_dir}" "${deb_args[*]}" "${rpm_args[*]}"
}

build_with_docker() {
    local triple="$1"
    local arch_label="$2"
    local platform="$3"
    local dockerfile="$4"
    local image="$5"
    local target_dir="target/${triple}"
    local release_dir="${ROOT}/${target_dir}/release"

    require_cmd docker
    mkdir -p "${release_dir}"

    echo "Building seriallcd ${CRATE_VERSION} (${arch_label}) via Docker (${platform})..."
    docker buildx build --platform "${platform}" --load -f "${dockerfile}" -t "${image}" "${ROOT}"
    cid=$(docker create "${image}")
    docker cp "${cid}:/usr/local/bin/seriallcd" "${release_dir}/seriallcd"
    docker rm "${cid}" >/dev/null

    local deb_args=(--no-build --target "${triple}")
    local rpm_args=(--target "${triple}")
    package_artifacts "${triple}" "${arch_label}" "${target_dir}" "${deb_args[*]}" "${rpm_args[*]}"
}

OUT_DIR="${ROOT}/releases/${CRATE_VERSION}"
mkdir -p "${OUT_DIR}"

for TARGET_TRIPLE in "${TARGETS[@]}"; do
    arch_label="$(derive_arch_label "${TARGET_TRIPLE}")"

    case "${TARGET_TRIPLE}" in
        "")
            build_with_cargo "" "${arch_label}"
            ;;
        arm-unknown-linux-musleabihf)
            build_with_docker "${TARGET_TRIPLE}" "${arch_label}" "linux/arm/v6" "docker/Dockerfile.armv6" "seriallcd:armv6"
            ;;
        armv7-unknown-linux-gnueabihf)
            build_with_docker "${TARGET_TRIPLE}" "${arch_label}" "linux/arm/v7" "docker/Dockerfile.armv7" "seriallcd:armv7"
            ;;
        aarch64-unknown-linux-gnu)
            build_with_docker "${TARGET_TRIPLE}" "${arch_label}" "linux/arm64/v8" "docker/Dockerfile.arm64" "seriallcd:arm64"
            ;;
        *)
            build_with_cargo "${TARGET_TRIPLE}" "${arch_label}"
            ;;
    esac
done

if [[ "${UPLOAD}" -eq 1 ]]; then
    require_cmd gh

    if ! git rev-parse "${RELEASE_TAG}" >/dev/null 2>&1; then
        echo "Git tag ${RELEASE_TAG} not found; create it before uploading." >&2
        exit 1
    fi

    echo "Uploading assets to GitHub release ${RELEASE_TAG}..."
    if gh release view "${RELEASE_TAG}" >/dev/null 2>&1; then
        gh release upload "${RELEASE_TAG}" "${UPLOAD_ASSETS[@]}" --clobber
    else
        gh release create "${RELEASE_TAG}" "${UPLOAD_ASSETS[@]}" \
            --title "seriallcd ${CRATE_VERSION}" \
            --notes "Local release build for seriallcd ${CRATE_VERSION}"
    fi
fi
