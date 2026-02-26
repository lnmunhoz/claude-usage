#!/usr/bin/env bash
set -euo pipefail

# ─── Paths ───────────────────────────────────────────────────────────────────
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
DIST_DIR="$SCRIPT_DIR/dist"
ARTIFACT="Token Juice.app.tar.gz"
SIG_FILE="$ARTIFACT.sig"

# ─── Find bundle directory (varies by build target) ─────────────────────────
BUNDLE_DIR=""
for candidate in \
  "$PROJECT_ROOT/src-tauri/target/aarch64-apple-darwin/release/bundle/macos" \
  "$PROJECT_ROOT/src-tauri/target/release/bundle/macos"; do
  if [ -f "$candidate/$ARTIFACT" ] && [ -f "$candidate/$SIG_FILE" ]; then
    BUNDLE_DIR="$candidate"
    break
  fi
done

# ─── Read current version from tauri.conf.json ──────────────────────────────
VERSION=$(node -e "console.log(JSON.parse(require('fs').readFileSync('$PROJECT_ROOT/src-tauri/tauri.conf.json')).version)")
echo "Current version in tauri.conf.json: $VERSION"

# ─── Check that build artifacts exist ────────────────────────────────────────
if [ -z "$BUNDLE_DIR" ]; then
  echo ""
  echo "Build artifacts not found. Looked in:"
  echo "  src-tauri/target/aarch64-apple-darwin/release/bundle/macos/"
  echo "  src-tauri/target/release/bundle/macos/"
  echo ""
  echo "Run 'pnpm build:mac' first, then re-run this script."
  exit 1
fi
echo "Found artifacts in: $BUNDLE_DIR"

# ─── Copy artifacts to dist/ ────────────────────────────────────────────────
mkdir -p "$DIST_DIR"
cp "$BUNDLE_DIR/$ARTIFACT" "$DIST_DIR/"
echo "Copied $ARTIFACT → dist/"

# ─── Read signature ─────────────────────────────────────────────────────────
SIGNATURE=$(cat "$BUNDLE_DIR/$SIG_FILE")

# ─── Generate latest.json ───────────────────────────────────────────────────
PUB_DATE=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

cat > "$DIST_DIR/latest.json" <<EOF
{
  "version": "$VERSION",
  "notes": "Local test update v$VERSION",
  "pub_date": "$PUB_DATE",
  "platforms": {
    "darwin-aarch64": {
      "signature": "$SIGNATURE",
      "url": "https://localhost:8443/$ARTIFACT"
    }
  }
}
EOF

echo "Generated dist/latest.json"
echo ""
echo "── dist/latest.json ──"
cat "$DIST_DIR/latest.json"
echo ""
echo "Done! Run 'pnpm update-server' to start the HTTPS server."
