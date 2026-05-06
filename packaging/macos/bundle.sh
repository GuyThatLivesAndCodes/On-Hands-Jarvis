#!/usr/bin/env bash
# Build a macOS .app bundle and a .dmg image from a release binary.
#
# Usage: packaging/macos/bundle.sh [VERSION]
#
# Run after `cargo build --release`. The script reads the version from
# Cargo.toml unless an argument is supplied.

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

VERSION="${1:-$(grep -m1 '^version' Cargo.toml | sed -E 's/.*"([^"]+)".*/\1/')}"
APP_NAME="On-Hands Jarvis"
BIN_NAME="on-hands-jarvis"
DIST="$ROOT/dist"
APP_DIR="$DIST/$APP_NAME.app"

echo ">>> Building $APP_NAME.app  (version $VERSION)"
rm -rf "$APP_DIR"
mkdir -p "$APP_DIR/Contents/MacOS"
mkdir -p "$APP_DIR/Contents/Resources"

# Locate the built binary. Allows for cross/lipo'd release dirs too.
BIN_PATH="target/release/$BIN_NAME"
if [[ ! -f "$BIN_PATH" ]]; then
    if [[ -f "target/aarch64-apple-darwin/release/$BIN_NAME" && -f "target/x86_64-apple-darwin/release/$BIN_NAME" ]]; then
        echo ">>> Creating universal binary"
        mkdir -p target/universal/release
        lipo -create -output "target/universal/release/$BIN_NAME" \
            "target/aarch64-apple-darwin/release/$BIN_NAME" \
            "target/x86_64-apple-darwin/release/$BIN_NAME"
        BIN_PATH="target/universal/release/$BIN_NAME"
    else
        echo "error: $BIN_PATH not found. Build with cargo first." >&2
        exit 1
    fi
fi

cp "$BIN_PATH" "$APP_DIR/Contents/MacOS/$BIN_NAME"
chmod +x "$APP_DIR/Contents/MacOS/$BIN_NAME"

sed "s/__VERSION__/$VERSION/g" "$ROOT/packaging/macos/Info.plist" > "$APP_DIR/Contents/Info.plist"
cp "$ROOT/packaging/macos/com.onhands.jarvis.plist" "$APP_DIR/Contents/Resources/"
cp "$ROOT/README.md" "$APP_DIR/Contents/Resources/" || true

# Drop a one-click autostart enabler into the staging dir; it ends up in
# the DMG next to the .app.
ENABLE_SCRIPT="$DIST/Enable Autostart.command"
cat > "$ENABLE_SCRIPT" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
DEST="$HOME/Library/LaunchAgents"
mkdir -p "$DEST"
SRC="/Applications/On-Hands Jarvis.app/Contents/Resources/com.onhands.jarvis.plist"
if [[ ! -f "$SRC" ]]; then
    echo "On-Hands Jarvis is not installed at /Applications. Drag the .app there first."
    exit 1
fi
cp "$SRC" "$DEST/com.onhands.jarvis.plist"
launchctl unload "$DEST/com.onhands.jarvis.plist" 2>/dev/null || true
launchctl load   "$DEST/com.onhands.jarvis.plist"
echo "Autostart enabled. Jarvis will launch at login."
EOF
chmod +x "$ENABLE_SCRIPT"

# Build a DMG if `hdiutil` is available (i.e. running on macOS).
if command -v hdiutil >/dev/null 2>&1; then
    DMG="$DIST/On-Hands-Jarvis-$VERSION.dmg"
    echo ">>> Building $DMG"
    rm -f "$DMG"
    STAGE="$(mktemp -d)"
    cp -R "$APP_DIR" "$STAGE/"
    cp "$ENABLE_SCRIPT" "$STAGE/"
    ln -s /Applications "$STAGE/Applications"
    hdiutil create -volname "$APP_NAME" -srcfolder "$STAGE" -ov -format UDZO "$DMG"
    rm -rf "$STAGE"
else
    echo ">>> Skipping DMG (hdiutil unavailable; not on macOS?)"
    # Tar the .app + helper script as a portable fallback.
    TARBALL="$DIST/On-Hands-Jarvis-$VERSION-macos.tar.gz"
    tar -C "$DIST" -czf "$TARBALL" "$APP_NAME.app" "Enable Autostart.command"
    echo ">>> Wrote $TARBALL"
fi

echo ">>> Done."
