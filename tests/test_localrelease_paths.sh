#!/usr/bin/env bash
set -euo pipefail

# Test local-release.sh decision paths without performing real builds by setting SKIP_BUILD_ACTIONS=1
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TMP="$ROOT/target/test_localrelease"
rm -rf "$TMP" && mkdir -p "$TMP/bin"

fake_path="$TMP/bin"

# Fake uname utility so we can simulate host arch
cat > "$fake_path/uname" <<'UNAME'
#!/usr/bin/env bash
echo "aarch64"
UNAME
chmod +x "$fake_path/uname"

# Fake rustup that can simulate installed targets or not based on $RUSTUP_INSTALLED
cat > "$fake_path/rustup" <<'RUSTUP'
#!/usr/bin/env bash
if [[ "$*" == "target list --installed" ]]; then
    if [[ "${RUSTUP_INSTALLED:-0}" == "1" ]]; then
        echo "aarch64-unknown-linux-gnu"
        exit 0
    fi
    exit 0
fi
exit 0
RUSTUP
chmod +x "$fake_path/rustup"

# Fake cargo and packaging tools so require_cmd passes in test mode
for tool in cargo cargo-deb cargo-generate-rpm python3; do
    cat > "$fake_path/$tool" <<TOOL
#!/usr/bin/env bash
echo "$tool invoked"
exit 0
TOOL
    chmod +x "$fake_path/$tool"
done

export PATH="$fake_path:$PATH"

echo "-- case: aarch64 host, rustup target not installed -> fallback to docker --"
RUSTUP_INSTALLED=0 SKIP_BUILD_ACTIONS=1 bash "$ROOT/scripts/local-release.sh" --target aarch64-unknown-linux-gnu 2>&1 | tee "$TMP/out1.txt"
grep -qi "falling back to Docker" "$TMP/out1.txt" || (cat "$TMP/out1.txt"; echo 'FAIL: expected fallback to Docker'; exit 2)

echo "-- case: aarch64 host, rustup target installed -> prefer host build --"
RUSTUP_INSTALLED=1 SKIP_BUILD_ACTIONS=1 bash "$ROOT/scripts/local-release.sh" --target aarch64-unknown-linux-gnu 2>&1 | tee "$TMP/out2.txt"
grep -qi "building natively with cargo" "$TMP/out2.txt" || (cat "$TMP/out2.txt"; echo 'FAIL: expected native cargo build'; exit 2)

echo "-- case: FORCE_DOCKER overrides host preference --"
RUSTUP_INSTALLED=1 FORCE_DOCKER=1 SKIP_BUILD_ACTIONS=1 bash "$ROOT/scripts/local-release.sh" --target aarch64-unknown-linux-gnu 2>&1 | tee "$TMP/out3.txt"
grep -qi "FORCE_DOCKER=1 set; using Docker" "$TMP/out3.txt" || (cat "$TMP/out3.txt"; echo 'FAIL: expected FORCE_DOCKER override'; exit 2)

echo "-- case: USE_HOST_BUILD=0 forces Docker even when rustup target present --"
RUSTUP_INSTALLED=1 USE_HOST_BUILD=0 SKIP_BUILD_ACTIONS=1 bash "$ROOT/scripts/local-release.sh" --target aarch64-unknown-linux-gnu 2>&1 | tee "$TMP/out4.txt"
grep -qi "Falling back to Docker" "$TMP/out4.txt" || (cat "$TMP/out4.txt"; echo 'FAIL: expected Docker fallback when USE_HOST_BUILD=0'; exit 2)

echo "local-release path tests OK"
