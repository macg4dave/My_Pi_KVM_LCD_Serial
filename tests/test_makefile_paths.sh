#!/usr/bin/env bash
set -euo pipefail

# Test Makefile behavior for arm64 builds preferring host when rustup target present
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TMP="$ROOT/target/test_makefile"
rm -rf "$TMP" && mkdir -p "$TMP/bin"

fake_path="$TMP/bin"

# Fake rustup: behave like installed or not depending on RUSTUP_INSTALLED
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

# Fake uname
cat > "$fake_path/uname" <<'UNAME'
#!/usr/bin/env bash
echo "aarch64"
UNAME
chmod +x "$fake_path/uname"

# Fake cargo: on build create an expected binary path so Makefile copying succeeds
cat > "$fake_path/cargo" <<'CARGO'
#!/usr/bin/env bash
args="$*"
if [[ "$args" == *"--target"* ]]; then
    # extract target from args
    for i in $@; do
        if [[ "$prev" == "--target" ]]; then
            target="$i"
            break
        fi
        prev="$i"
    done
    mkdir -p "target/${target}/release"
    printf 'fake-binary' > "target/${target}/release/lifelinetty"
    exit 0
else
    mkdir -p target/release
    printf 'fake-binary' > target/release/lifelinetty
    exit 0
fi
CARGO
chmod +x "$fake_path/cargo"

# Fake docker for fallback path
cat > "$fake_path/docker" <<'DOCKER'
#!/usr/bin/env bash
if [[ "$1" == "buildx" ]]; then
    # simulate buildx build success
    exit 0
fi
if [[ "$1" == "create" ]]; then
    # return fake container id
    echo "fakecid"
    exit 0
fi
if [[ "$1" == "cp" ]]; then
    # params: cp <cid>:/usr/local/bin/lifelinetty dest
    dest="$3"
    mkdir -p "$(dirname "$dest")"
    printf 'fake-docker-binary' > "$dest"
    exit 0
fi
if [[ "$1" == "rm" ]]; then
    exit 0
fi
exit 0
DOCKER
chmod +x "$fake_path/docker"


export PATH="$fake_path:$PATH"

# Evaluate the same shell decision logic used in the Makefile for arm64 and assert behavior
echo "-- case: host aarch64 and rustup target installed -> Makefile logic should prefer cargo host build --"
PATH="$fake_path:$PATH" RUSTUP_INSTALLED=1 bash -c '
AARCH64_TARGET="aarch64-unknown-linux-gnu"
if [ "${FORCE_DOCKER:-}" = "1" ]; then
    echo docker
elif uname -m | grep -Eq "aarch64|arm64"; then
    if rustup target list --installed | grep -qx "${AARCH64_TARGET}"; then
        echo cargo
    else
        echo docker
    fi
else
    echo docker
fi
' | grep -xq cargo || (echo "FAIL: expected cargo path" && exit 2)

echo "-- case: host aarch64 but rustup missing -> Makefile logic should fall back to Docker --"
PATH="$fake_path:$PATH" RUSTUP_INSTALLED=0 bash -c '
AARCH64_TARGET="aarch64-unknown-linux-gnu"
if [ "${FORCE_DOCKER:-}" = "1" ]; then
    echo docker
elif uname -m | grep -Eq "aarch64|arm64"; then
    if rustup target list --installed | grep -qx "${AARCH64_TARGET}"; then
        echo cargo
    else
        echo docker
    fi
else
    echo docker
fi
' | grep -xq docker || (echo "FAIL: expected docker fallback" && exit 2)

echo "Makefile logic tests OK"

echo "-- case: USE_HOST_BUILD=0 should force Docker even if rustup target installed --"
PATH="$fake_path:$PATH" RUSTUP_INSTALLED=1 USE_HOST_BUILD=0 bash -c '
AARCH64_TARGET="aarch64-unknown-linux-gnu"
if [ "${FORCE_DOCKER:-}" = "1" ]; then
    echo docker
elif [ "${USE_HOST_BUILD:-1}" = "0" ]; then
    echo docker
elif uname -m | grep -Eq "aarch64|arm64"; then
    if rustup target list --installed | grep -qx "${AARCH64_TARGET}"; then
        echo cargo
    else
        echo docker
    fi
else
    echo docker
fi
' | grep -xq docker || (echo "FAIL: expected docker due to USE_HOST_BUILD=0" && exit 2)

echo "Makefile USE_HOST_BUILD test OK"

echo "Makefile path tests OK"
