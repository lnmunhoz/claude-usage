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
HELPER_DEST="${HOME}/.local/share/token-juice/token_juice_helper.py"
PLASMOID_PKG="${SCRIPT_DIR}/package"
PLASMOID_ID="com.tokenjuice.plasmoid"

# ---------- helpers ----------

info()  { echo -e "\033[1;34m[token-juice]\033[0m $*"; }
ok()    { echo -e "\033[1;32m[token-juice]\033[0m $*"; }
err()   { echo -e "\033[1;31m[token-juice]\033[0m $*" >&2; }

check_deps() {
    local missing=()
    command -v python3  >/dev/null 2>&1 || missing+=("python3")
    command -v kpackagetool6 >/dev/null 2>&1 || missing+=("kpackagetool6 (plasma-sdk)")

    if [[ ${#missing[@]} -gt 0 ]]; then
        err "Missing dependencies: ${missing[*]}"
        exit 1
    fi

    # Check Python packages
    local py_missing=()
    python3 -c "import rookiepy" 2>/dev/null || py_missing+=("rookiepy")
    python3 -c "import requests" 2>/dev/null || py_missing+=("requests")

    if [[ ${#py_missing[@]} -gt 0 ]]; then
        info "Installing missing Python packages: ${py_missing[*]}"
        pip install --user "${py_missing[@]}" || {
            err "Failed to install Python packages. Please install manually:"
            err "  pip install ${py_missing[*]}"
            exit 1
        }
    fi
}

# ---------- install ----------

do_install() {
    check_deps

    # Install helper script
    info "Installing helper script to ${HELPER_DEST}..."
    mkdir -p "$(dirname "${HELPER_DEST}")"
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

    info "Removing helper script..."
    rm -f "${HELPER_DEST}"
    rmdir --ignore-fail-on-non-empty "$(dirname "${HELPER_DEST}")" 2>/dev/null || true
    ok "Helper script removed."

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
