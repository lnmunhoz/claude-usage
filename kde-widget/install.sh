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
HELPER_SRC="${SCRIPT_DIR}/helpers/token_juice_helper.py"
INSTALL_DIR="${HOME}/.local/share/token-juice"
HELPER_DEST="${INSTALL_DIR}/token_juice_helper.py"
VENV_DIR="${INSTALL_DIR}/venv"
PLASMOID_PKG="${SCRIPT_DIR}/package"
PLASMOID_ID="com.tokenjuice.plasmoid"

# ---------- helpers ----------

info()  { echo -e "\033[1;34m[token-juice]\033[0m $*"; }
ok()    { echo -e "\033[1;32m[token-juice]\033[0m $*"; }
err()   { echo -e "\033[1;31m[token-juice]\033[0m $*" >&2; }

check_system_deps() {
    local missing=()
    command -v python3  >/dev/null 2>&1 || missing+=("python3")
    command -v kpackagetool6 >/dev/null 2>&1 || missing+=("kpackagetool6 (plasma-sdk)")

    if [[ ${#missing[@]} -gt 0 ]]; then
        err "Missing system dependencies: ${missing[*]}"
        exit 1
    fi
}

setup_venv() {
    if [[ ! -d "${VENV_DIR}" ]]; then
        info "Creating Python virtual environment at ${VENV_DIR}..."
        python3 -m venv "${VENV_DIR}"
    fi

    local pip="${VENV_DIR}/bin/pip"

    # Upgrade pip quietly
    "${pip}" install --upgrade pip -q 2>/dev/null || true

    # Install/upgrade required packages
    local py_missing=()
    "${VENV_DIR}/bin/python" -c "import rookiepy" 2>/dev/null || py_missing+=("rookiepy")
    "${VENV_DIR}/bin/python" -c "import requests" 2>/dev/null || py_missing+=("requests")

    if [[ ${#py_missing[@]} -gt 0 ]]; then
        info "Installing Python packages into venv: ${py_missing[*]}"

        # rookiepy uses PyO3 (Rust-Python bindings) which may not officially
        # support the latest Python yet. The stable ABI forward-compat flag
        # lets it build anyway (recommended by PyO3 docs).
        export PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1

        "${pip}" install "${py_missing[@]}" || {
            err "Failed to install Python packages."
            err "Try manually:"
            err "  export PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1"
            err "  ${VENV_DIR}/bin/pip install rookiepy requests"
            exit 1
        }
    fi

    ok "Python venv ready (${VENV_DIR})"
}

# ---------- install ----------

do_install() {
    check_system_deps

    # Create install directory
    mkdir -p "${INSTALL_DIR}"

    # Set up venv with dependencies
    setup_venv

    # Install helper script
    info "Installing helper script to ${HELPER_DEST}..."
    cp "${HELPER_SRC}" "${HELPER_DEST}"
    chmod +x "${HELPER_DEST}"
    ok "Helper script installed."

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

    info "Removing helper script and venv..."
    rm -rf "${INSTALL_DIR}"
    ok "Helper and venv removed."

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
