#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
DIST_ROOT="${REPO_ROOT}/dist"
PLAYTEST_ROOT="${REPO_ROOT}/playtest"
PACKAGE_NAME="MoleGame-LinuxFriendPlaytest"
PACKAGE_ROOT="${DIST_ROOT}/${PACKAGE_NAME}"
TARBALL_PATH="${PLAYTEST_ROOT}/${PACKAGE_NAME}.tar.gz"

if [[ -d "${HOME}/.cargo/bin" ]]; then
    export PATH="${HOME}/.cargo/bin:${PATH}"
fi
if [[ -d "/usr/local/cargo/bin" ]]; then
    export PATH="/usr/local/cargo/bin:${PATH}"
fi

remove_inside_dist() {
    local path="$1"
    if [[ ! -e "${path}" ]]; then
        return
    fi

    local resolved_dist
    local resolved_path
    resolved_dist="$(realpath "${DIST_ROOT}")"
    resolved_path="$(realpath "${path}")"
    case "${resolved_path}" in
        "${resolved_dist}"/*) rm -rf -- "${resolved_path}" ;;
        *) echo "Refusing to remove path outside dist/: ${resolved_path}" >&2; exit 1 ;;
    esac
}

mkdir -p -- "${DIST_ROOT}" "${PLAYTEST_ROOT}"
remove_inside_dist "${PACKAGE_ROOT}"
rm -f -- "${TARBALL_PATH}"
mkdir -p -- "${PACKAGE_ROOT}"

(
    cd "${REPO_ROOT}"
    cargo build -p mole_runtime --release --features "sdl-bundled wup"
)

RUNTIME_BIN="${REPO_ROOT}/target/release/mole_runtime"
if [[ ! -x "${RUNTIME_BIN}" ]]; then
    echo "mole_runtime was not built at ${RUNTIME_BIN}" >&2
    exit 1
fi

cp -- "${RUNTIME_BIN}" "${PACKAGE_ROOT}/mole_runtime"
cp -R -- "${REPO_ROOT}/DolphinMole" "${PACKAGE_ROOT}/DolphinMole"

cat > "${PACKAGE_ROOT}/Run Mole Game.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")"
export MOLE_ASSET_ROOT="${PWD}"
exec ./mole_runtime --friend-connect --play --netplay-delay 2 "$@"
EOF

cat > "${PACKAGE_ROOT}/Run Mole Game Trace.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")"
export MOLE_ASSET_ROOT="${PWD}"
exec ./mole_runtime --friend-connect --play --input-trace --netplay-delay 2 "$@"
EOF

cat > "${PACKAGE_ROOT}/Run Mole Game Vanilla No UCF.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")"
export MOLE_ASSET_ROOT="${PWD}"
exec ./mole_runtime --friend-connect --play --no-ucf --netplay-delay 2 "$@"
EOF

cat > "${PACKAGE_ROOT}/Run Local Practice.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")"
export MOLE_ASSET_ROOT="${PWD}"
exec ./mole_runtime --sdl --play "$@"
EOF

cat > "${PACKAGE_ROOT}/Run Local Practice Vanilla No UCF.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")"
export MOLE_ASSET_ROOT="${PWD}"
exec ./mole_runtime --sdl --play --no-ucf "$@"
EOF

cat > "${PACKAGE_ROOT}/Check WUP Adapter.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")"
exec ./mole_runtime --check-wup "$@"
EOF

cat > "${PACKAGE_ROOT}/README.md" <<'EOF'
# Mole Game Linux / Steam Deck Friend Playtest

This folder is a Linux playtest package for the native Rust SDL3/WUP runtime.
It is built for x86_64 Linux with bundled SDL3 and desktop SDL video backends,
so it is intended to run on Steam Deck under SteamOS Desktop Mode or as an
added non-Steam game.

## Requirements

- A desktop session that can open SDL windows.
- Optional WUP-028/GameCube adapter access through normal Linux USB permissions.

## Launchers

- `./Run Mole Game.sh`: Friend Connect playtest with UCF enabled.
- `./Run Mole Game Trace.sh`: Friend Connect with controller/core input trace JSONL enabled.
- `./Run Mole Game Vanilla No UCF.sh`: Friend Connect with UCF disabled.
- `./Run Local Practice.sh`: local single-machine SDL practice.
- `./Run Local Practice Vanilla No UCF.sh`: local practice with UCF disabled.
- `./Check WUP Adapter.sh`: checks whether the native adapter path can see the adapter.

Friend Connect uses Supabase only for endpoint setup. Gameplay packets are direct
UDP between peers and use the same two-frame delay/repair buffer as the Windows
playtest package. This Linux package can connect to the Windows Friend Connect
playtest when both clients can reach each other over UDP.
EOF

chmod +x \
    "${PACKAGE_ROOT}/mole_runtime" \
    "${PACKAGE_ROOT}/Run Mole Game.sh" \
    "${PACKAGE_ROOT}/Run Mole Game Trace.sh" \
    "${PACKAGE_ROOT}/Run Mole Game Vanilla No UCF.sh" \
    "${PACKAGE_ROOT}/Run Local Practice.sh" \
    "${PACKAGE_ROOT}/Run Local Practice Vanilla No UCF.sh" \
    "${PACKAGE_ROOT}/Check WUP Adapter.sh"

tar -czf "${TARBALL_PATH}" -C "${DIST_ROOT}" "${PACKAGE_NAME}"

echo "Created Linux package folder: ${PACKAGE_ROOT}"
echo "Created Linux playtest tarball: ${TARBALL_PATH}"
