#!/usr/bin/env bash
#
# Token Juice KDE Plasma Widget -- Installer
#
# Usage:
#   ./install.sh          # Install (or upgrade)
#   ./install.sh remove   # Uninstall
#
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
HELPER_DIR="${SCRIPT_DIR}/helpers"
INSTALL_DIR="${HOME}/.local/share/token-juice"
HELPER_DEST="${INSTALL_DIR}/token-juice-helper"
PLASMOID_PKG="${SCRIPT_DIR}/package"
PLASMOID_ID="com.tokenjuice.plasmoid"

# ---------- helpers ----------

info()  { echo -e "\033[1;34m[token-juice]\033[0m $*"; }
ok()    { echo -e "\033[1;32m[token-juice]\033[0m $*"; }
err()   { echo -e "\033[1;31m[token-juice]\033[0m $*" >&2; }

check_system_deps() {
    local missing=()
    command -v cargo >/dev/null 2>&1 || missing+=("cargo (rustup)")
    command -v kpackagetool6 >/dev/null 2>&1 || missing+=("kpackagetool6 (plasma-sdk)")

    if [[ ${#missing[@]} -gt 0 ]]; then
        err "Missing system dependencies: ${missing[*]}"
        exit 1
    fi
}

build_helper() {
    info "Building Rust helper binary..."
    cargo build --release --manifest-path "${HELPER_DIR}/Cargo.toml" || {
        err "Failed to build Rust helper binary."
        err "Make sure you have a working Rust toolchain (rustup.rs)."
        exit 1
    }
    ok "Rust helper built successfully."
}

# ---------- install ----------

do_install() {
    check_system_deps

    # Create install directory
    mkdir -p "${INSTALL_DIR}"

    # Build the Rust helper
    build_helper

    # Install helper binary
    info "Installing helper binary to ${HELPER_DEST}..."
    cp "${HELPER_DIR}/target/release/token-juice-helper" "${HELPER_DEST}"
    chmod +x "${HELPER_DEST}"
    ok "Helper binary installed."

    # Install plasmoid
    info "Installing plasmoid..."
    if kpackagetool6 -t Plasma/Applet -s "${PLASMOID_ID}" >/dev/null 2>&1; then
        info "Upgrading existing plasmoid..."
        kpackagetool6 -t Plasma/Applet -u "${PLASMOID_PKG}"
    else
        kpackagetool6 -t Plasma/Applet -i "${PLASMOID_PKG}"
    fi
    ok "Plasmoid installed."

    echo ""
    ok "Installation complete!"
    info "Add 'Token Juice' widget to your desktop or panel."
    info "Right-click desktop -> Add Widgets -> search for 'Token Juice'"
    echo ""
}

# ---------- remove ----------

do_remove() {
    info "Removing plasmoid..."
    kpackagetool6 -t Plasma/Applet -r "${PLASMOID_ID}" 2>/dev/null || true
    ok "Plasmoid removed."

    info "Removing helper binary..."
    rm -rf "${INSTALL_DIR}"
    ok "Helper removed."

    echo ""
    ok "Uninstall complete."
}

# ---------- main ----------

case "${1:-install}" in
    install)  do_install ;;
    remove|uninstall)  do_remove ;;
    *)
        echo "Usage: $0 [install|remove]"
        exit 1
        ;;
esac
