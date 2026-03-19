#!/usr/bin/env bash
set -euo pipefail

# Build keypo-signer as a .app bundle.
#
# Usage:
#   scripts/build-app-bundle.sh                    # Full build: compile + bundle + sign + notarize
#   scripts/build-app-bundle.sh --bundle-only      # Bundle only: compile + bundle (no signing, for local dev)
#
# Required environment variables (full mode only):
#   DEVELOPER_ID_CERT_NAME    — e.g., "Developer ID Application: Keypo Inc (TEAM123)"
#   NOTARIZATION_APPLE_ID     — Apple ID email
#   NOTARIZATION_TEAM_ID      — Apple Developer Team ID
#   NOTARIZATION_APP_PASSWORD — App-specific password

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
SIGNER_DIR="$REPO_ROOT/keypo-signer"
BUILD_DIR="$SIGNER_DIR/.build/release"
APP_NAME="keypo-signer.app"
APP_DIR="$SIGNER_DIR/$APP_NAME"

BUNDLE_ONLY=false
if [[ "${1:-}" == "--bundle-only" ]]; then
    BUNDLE_ONLY=true
fi

echo "==> Building keypo-signer (release)..."
cd "$SIGNER_DIR"
swift build -c release

echo "==> Creating .app bundle..."
rm -rf "$APP_DIR"
mkdir -p "$APP_DIR/Contents/MacOS"

cp "$BUILD_DIR/keypo-signer" "$APP_DIR/Contents/MacOS/keypo-signer"
cp "$SIGNER_DIR/Info.plist" "$APP_DIR/Contents/Info.plist"

# Copy provisioning profile if available
if [[ -f "$SIGNER_DIR/embedded.provisionprofile" ]]; then
    cp "$SIGNER_DIR/embedded.provisionprofile" "$APP_DIR/Contents/embedded.provisionprofile"
fi

echo "==> .app bundle created at: $APP_DIR"

if $BUNDLE_ONLY; then
    echo "==> Bundle-only mode: skipping code-signing and notarization."
    exit 0
fi

# --- Full mode: sign and notarize ---

if [[ -z "${DEVELOPER_ID_CERT_NAME:-}" ]]; then
    echo "ERROR: DEVELOPER_ID_CERT_NAME not set" >&2
    exit 1
fi

echo "==> Code-signing .app bundle..."
codesign --force --sign "$DEVELOPER_ID_CERT_NAME" \
    --entitlements "$SIGNER_DIR/keypo-signer.entitlements" \
    --options runtime \
    --timestamp \
    "$APP_DIR"

echo "==> Verifying signature..."
codesign --verify --deep --strict "$APP_DIR"

echo "==> Submitting for notarization..."
ZIP_PATH="$SIGNER_DIR/.build/keypo-signer-notarize.zip"
ditto -c -k --keepParent "$APP_DIR" "$ZIP_PATH"

xcrun notarytool submit "$ZIP_PATH" \
    --apple-id "$NOTARIZATION_APPLE_ID" \
    --team-id "$NOTARIZATION_TEAM_ID" \
    --password "$NOTARIZATION_APP_PASSWORD" \
    --wait --timeout 600

echo "==> Stapling notarization ticket..."
xcrun stapler staple "$APP_DIR"

echo "==> Done. Notarized .app bundle: $APP_DIR"
