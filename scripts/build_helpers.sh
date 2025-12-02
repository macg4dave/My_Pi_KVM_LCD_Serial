#!/usr/bin/env bash
set -euo pipefail

# Helper functions used by local-release.sh and tests

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

# Returns 0 when running inside a container (Docker / Podman / systemd-nspawn)
is_inside_container() {
    if [[ -f '/.dockerenv' ]] || [[ -f '/run/.containerenv' ]]; then
        return 0
    fi

    if grep -qE 'docker|podman|containerd|lxc' /proc/1/cgroup 2>/dev/null; then
        return 0
    fi

    return 1
}

# Check if a Rust target is installed
has_rust_target_installed() {
    local target="$1"
    if rustup target list --installed | grep -qx "${target}"; then
        return 0
    fi
    return 1
}

export -f derive_arch_label is_inside_container has_rust_target_installed
