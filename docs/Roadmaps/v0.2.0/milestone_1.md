# Milestone 1 — Docker-to-docker devtest loop (build job)

> Charter-aligned: single Rust daemon, UART-only, stable CLI, cache at `/run/serial_lcd_cache`, persistence only at `~/.serial_lcd/config.toml`, no new flags/transports.
>
> Audience: operators, CI, and AI assistants. Goal is zero-drift, hardware-free regression runs using paired containers while keeping artifacts inside the cache bind mount.

## What “done” means

- Paired containers (local runner + remote/SSHD target) exercise the full `devtest/run-dev.sh` loop without hardware.
- All artifacts live under `/run/serial_lcd_cache` (bind or tmpfs) plus per-container `~/.serial_lcd`.
- Both `docker run` and docker-compose examples work.
- Scenarios from `devtest/config/*.toml` (or `--config-file`) are usable with the loop.
- Logs for each scenario land in `/run/serial_lcd_cache/milestone1/<scenario>-YYYYMMDD/` and are easy to `docker cp` out.
- CI/headless recipe exists; AI/debug instructions explain how to re-run and collect evidence.

## Guardrails to keep

- No new CLI flags; use existing: `--run --test-lcd --test-serial --device --baud --cols --rows --demo --serialsh --wizard --config-file`.
- Storage: only `/run/serial_lcd_cache` (RAM-disk or bind) and `~/.serial_lcd/config.toml` persist; nowhere else.
- Transports: UART newline JSON / `key=value`; LCD: HD44780 + PCF8574 @ 0x27. SSH is orchestration only.
- RSS < 5 MB, no busy loops; respect backoff/reconnect timing.

## Container topology

- **Remote container (target):** Runs SSHD, hosts the remote lifelinetty binary. Mount shared cache and its own `~/.serial_lcd`.
- **Local container (runner):** Invokes `devtest/run-dev.sh`, also mounts the same cache, keeps its own `~/.serial_lcd`.
- Shared **cache** bind (or tmpfs) at `/run/serial_lcd_cache` mounted into both containers.
- Optional: mount prebuilt binaries or `releases/debug/<arch>/lifelinetty` into both.

## Quickstart with `docker run`

```bash
# Shared cache
mkdir -p /tmp/lifelinetty-cache

# Remote (SSHD + cache + config)
docker run -d --name lifelinetty-remote \
  -p 2222:22 \
  -e PUID=0 -e PGID=0 -e TZ=UTC \
  -e PASSWORD_ACCESS=true -e USER_PASSWORD=devpass -e USER_NAME=root \
  -v /tmp/lifelinetty-cache:/run/serial_lcd_cache \
  -v lifelinetty-remote-home:/root \
  lscr.io/linuxserver/openssh-server:latest  # or your SSHD base

# Local runner (shares cache, mounts workspace read-only; adjust path)
docker run -it --name lifelinetty-local --rm \
  -v /home/dave/github/LifelineTTY:/workspace:ro \
  -v /tmp/lifelinetty-cache:/run/serial_lcd_cache \
  -w /workspace \
  debian:stable-slim bash

# linuxserver/openssh-server is configured for dev-only password auth: USER_NAME=root, USER_PASSWORD=devpass.

# Inside lifelinetty-local
apt-get update && apt-get install -y ssh scp curl ca-certificates git
cp devtest/dev.conf.example devtest/dev.conf
# Edit dev.conf:
#   PI_HOST=localhost
#   PI_USER=root
#   PI_BIN=/opt/lifelinetty/lifelinetty
#   LOCAL_BIN_SOURCE=/workspace/target/debug/lifelinetty (if prebuilt)
#   REMOTE_BIN_SOURCE=/workspace/target/debug/lifelinetty (optional)
./devtest/run-dev.sh
```

## Quickstart with docker-compose (recommended)

```yaml
# docker-compose.milestone1.yml (checked into repo root)
version: "3.8"
services:
  remote:
    image: lscr.io/linuxserver/openssh-server:latest
    container_name: lifelinetty-remote
    environment:
      - PUID=0
      - PGID=0
      - TZ=UTC
      - PASSWORD_ACCESS=true
      - USER_PASSWORD=devpass
      - USER_NAME=root
    ports: ["2222:22"]
    volumes:
      - cache:/run/serial_lcd_cache
      - remote-home:/root
    restart: unless-stopped
  local:
    image: debian:stable-slim
    container_name: lifelinetty-local
    working_dir: /workspace
    command: ["bash"]
    tty: true
    stdin_open: true
    volumes:
      - ./:/workspace:ro
      - cache:/run/serial_lcd_cache
    depends_on:
      - remote
volumes:
  cache:
  remote-home:
```

Then:

```bash
docker compose -f docker-compose.milestone1.yml up -d
# Exec into local runner (password auth is enabled for dev only: USER_NAME=root, USER_PASSWORD=devpass)
docker exec -it lifelinetty-local bash
apt-get update && apt-get install -y ssh scp curl ca-certificates git
cp devtest/dev.conf.example devtest/dev.conf
# Set PI_HOST=remote, PI_USER=root, PI_BIN=/opt/lifelinetty/lifelinetty
TERMINAL_CMD="" ENABLE_SSH_SHELL=false ./devtest/run-dev.sh
```

## Dev loop knobs (run-dev.sh)

Key envs in `devtest/dev.conf`:

- `PI_HOST`, `PI_USER`, `PI_BIN` — remote SSH target and binary path.
- `LOCAL_BIN_SOURCE` / `REMOTE_BIN_SOURCE` — optional prebuilt binary paths to upload instead of rebuilding.
- `REMOTE_ARCH` / `LOCAL_ARCH` — pull from `releases/debug/<arch>/lifelinetty` if set.
- `COMMON_ARGS` — defaults: `--run --device /dev/ttyUSB0 --baud 9600 --cols 16 --rows 2`.
- `REMOTE_ARGS`, `LOCAL_ARGS` — per-side overrides; you can pass `--config-file <path>` here.
- `CONFIG_SOURCE_FILE`, `LOCAL_CONFIG_SOURCE_FILE`, `REMOTE_CONFIG_SOURCE_FILE` — scenario templates copied into `~/.serial_lcd/config.toml`.
- `SCENARIO_NAME`, `SCENARIO_DATE`, `SCENARIO_DIR` — tag per-run bundles; logs land under `/run/serial_lcd_cache/milestone1/<scenario>-YYYYMMDD/{local,remote}` with `LIFELINETTY_LOG_PATH` set automatically for both sides.
- `TERMINAL_CMD` — GUI terminals; leave empty in headless/CI to run inline. Set `ENABLE_SSH_SHELL=false` to skip the interactive shell pane when headless.
- `ENABLE_LOG_PANE`, `LOG_WATCH_CMD` — optional log watcher (defaults to `watch -n 0.5 ls -lh $SCENARIO_DIR`).
- `PKILL_PATTERN` — kills stale remote lifelinetty before launch.

Pre-flight expectations:

- SSH reachable (`ssh -o BatchMode=yes -o ConnectTimeout=5`).
- Cache mount exists and writable in both containers.
- `~/.serial_lcd` exists in both; templates copied there unless `--config-file` is used.
- `lifelinetty.service` must be off in the remote container (if present) to avoid TTY contention.

Launch behavior:

- Creates temp HOME for local side, copies chosen template, builds (or uses provided binary), scps to remote, chmod + pkill stale processes, then launches remote/local commands (and optional log watcher). Each side sets `LIFELINETTY_LOG_PATH` inside the scenario bundle at `/run/serial_lcd_cache/milestone1/<scenario>-YYYYMMDD/{local,remote}`. Titles: SSH / Remote / Local / Logs. In headless/CI runs (`TERMINAL_CMD=""`), the script backgrounds processes and can skip the SSH pane via `ENABLE_SSH_SHELL=false`.

## Test matrix for this milestone

Log each run under `/run/serial_lcd_cache/milestone1/<scenario>-YYYYMMDD/` and `docker cp` out if needed.

- **Baseline**: `/dev/ttyUSB0` @ 9600, 16×2, payload `samples/payload_examples.json`.
- **Alt TTY**: `/dev/ttyAMA0` @ 9600, 16×2.
- **20×4 geometry**: `/dev/ttyUSB0` @ 9600, 20×4 template.
- **Higher baud probe**: `/dev/ttyUSB0` @ 19200, after 9600 is stable (stress template).
- **Demo/no-serial smoke**: `--demo` to validate render loop without UART.

Expected checks: clean render (no flicker), backoff logs in cache only, RSS < 5 MB, reconnect counters present, CGRAM churn ≤ 8 slots, tunnels unaffected by render loop.

## CI/headless recipe (AI-friendly)

- Use compose file above with no `TERMINAL_CMD`; run inline:

  ```bash
  REMOTE_ARCH=armv6 LOCAL_ARCH=armv6 \
  PI_HOST=remote PI_USER=root PI_BIN=/opt/lifelinetty/lifelinetty \
  COMMON_ARGS="--run --device /dev/ttyUSB0 --baud 9600 --cols 16 --rows 2" \
  ./devtest/run-dev.sh
  ```

- Collect artifacts for the job:
  - `/run/serial_lcd_cache/milestone1/**`
  - `/run/serial_lcd_cache/{serial_backoff,watchdog,tunnel,wizard,polling}/*.log`
- Publish as CI artifacts; do not write outside cache.

## AI re-run + debug checklist

When asked to rerun or debug:

1. Ensure cache mount exists: `ls -l /run/serial_lcd_cache` (inside each container).
2. Verify SSH: `ssh -o BatchMode=yes -p 22 root@remote true` (or port 2222 if mapped).
3. Rebuild if needed: run whatever `BUILD_CMD` you configured (default `make all`, which writes `releases/debug/<arch>/lifelinetty` for each target). You can also point `LOCAL_BIN_SOURCE`/`REMOTE_BIN_SOURCE` at an existing release artifact if you skip the build step.
4. Run the loop headless: `TERMINAL_CMD="" ./devtest/run-dev.sh` with correct envs.
5. Fetch logs: `docker cp lifelinetty-remote:/run/serial_lcd_cache ./cache-copy` (or from local if shared bind).
6. Inspect key logs: `serial_backoff.log`, `watchdog.log`, scenario folder under `milestone1/`.
7. Run unit tests locally for parity: `cargo fmt && cargo clippy -- -D warnings && cargo test`.

## Troubleshooting

- **SSH fails**: check port mapping (2222→22), container name (`PI_HOST=remote`), and authorized_keys.
- **Cache missing**: ensure `-v /tmp/lifelinetty-cache:/run/serial_lcd_cache` (or compose volume) on both containers.
- **Binary not executable**: confirm scp path and `chmod +x $PI_BIN` (script does this).
- **TTY contention**: stop any systemd service using the TTY; remote container should not auto-start lifelinetty.
- **No GUI terminals**: leave `TERMINAL_CMD` empty; script runs inline.

## Acceptance checklist for Milestone 1 doc

- Includes `docker run` and compose examples with cache/config mounts.
- Documents `devtest/run-dev.sh` knobs, templates, and `--config-file` option.
- Provides test matrix + expected log placement under `/run/serial_lcd_cache/milestone1/`.
- Describes CI/headless flow and AI re-run instructions.
- Respects charter guardrails (no new flags/transports; cache/config paths only).

## Status & evidence

Milestone 1 is complete once the documented build job can be executed and audited entirely under `/run/serial_lcd_cache` and `~/.serial_lcd/config.toml`. The following sections provide the evidence that each requirement is satisfied:

| Requirement | Evidence |
| --- | --- |
| Container topology + quickstart | "Container topology", "Quickstart with `docker run`", and "Quickstart with docker-compose" sections describe the paired containers, shared cache, and verbatim `docker run` / compose commands that operators should follow. |
| Dev loop knobs & templates | "Dev loop knobs" lists the key `devtest/dev.conf` knobs, and `devtest/run-dev.sh` copies the referenced templates (`devtest/config/*.toml`) into each `~/.serial_lcd/config.toml`. The doc references `--config-file` overrides for scenarios as well. |
| Test matrix & log placement | "Test matrix" explicitly enumerates the baseline, alt-TTY, 20×4, higher baud, and `--demo` scenarios plus the requirement to log each run under `/run/serial_lcd_cache/milestone1/<scenario>-YYYYMMDD` for easy `docker cp` retrieval. |
| CI/headless recipe & AI rerun checklist | "CI/headless recipe" describes the env vars for headless invocations, and "AI re-run + debug checklist" lists the steps an AI or operator should repeat along with the cache/log collection commands. |
| Charter guardrails & troubleshooting | The opening summary, guardrails, and "Troubleshooting" section reiterate the `/run/serial_lcd_cache` + `~/.serial_lcd/config.toml` storage policy, stable flags, and RSS/logging expectations mandated by the charter. |

With these references in place, the roadmap can now consider Milestone 1 fulfilled for release planning, and every artifact required for the build-based devtest loop is described for operators, CI, and AI assistants.
