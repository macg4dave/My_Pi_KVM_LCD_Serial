#!/usr/bin/env bash
set -euo pipefail

# Quick unit tests for scripts/build_helpers.sh helper functions
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
source "${ROOT}/scripts/build_helpers.sh"

echo "Testing derive_arch_label..."
[[ "$(derive_arch_label arm-unknown-linux-musleabihf)" == "armv6" ]]
[[ "$(derive_arch_label armv7-unknown-linux-gnueabihf)" == "armv7" ]]
[[ "$(derive_arch_label aarch64-unknown-linux-gnu)" == "arm64" ]]
[[ "$(derive_arch_label "armv7-foo")" == "armv7" ]]

echo "Testing has_rust_target_installed (mock rustup)..."
TMPDIR=$(mktemp -d)
cleanup() { rm -rf "$TMPDIR"; }
trap cleanup EXIT

# Create fake rustup
cat > "$TMPDIR/rustup" <<'RUSTUP'
#!/usr/bin/env bash
if [[ "$*" == "target list --installed" ]]; then
    echo "aarch64-unknown-linux-gnu"
    echo "armv7-unknown-linux-gnueabihf"
    exit 0
fi
exit 0
RUSTUP
chmod +x "$TMPDIR/rustup"

export PATH="$TMPDIR:$PATH"

has_rust_target_installed aarch64-unknown-linux-gnu
if [[ $? -ne 0 ]]; then
    echo "Expected aarch64 target to be detected" >&2
    exit 2
fi

! has_rust_target_installed not-installed-target && echo "not-installed-target correctly reported missing"

echo "All build_helpers tests passed."
