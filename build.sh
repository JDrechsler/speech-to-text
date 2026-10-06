#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")"

APP_NAME="SpeechToText"
DIST_DIR="dist"
BUNDLE="$DIST_DIR/$APP_NAME.app"
INSTALL_DIR="/Applications/$APP_NAME.app"
SIGNING_IDENTITY_NAME="SpeechToText Dev"

case "${1:-}" in
    ""|--run|--dist) ;;
    *)
        echo "Usage: ./build.sh [--run | --dist]"
        echo "  (no flag)  build and install to /Applications"
        echo "  --run      build, install and launch"
        echo "  --dist     build $DIST_DIR/$APP_NAME.zip for sharing, install nothing"
        exit 1
        ;;
esac

swift build -c release
BIN_DIR="$(swift build -c release --show-bin-path)"

ADAPTER_DIR="vendor/mediaremote-adapter"
ADAPTER_FRAMEWORK="$ADAPTER_DIR/build/MediaRemoteAdapter.framework"
if [[ ! -f "$ADAPTER_FRAMEWORK/MediaRemoteAdapter" ]]; then
    echo "Building MediaRemoteAdapter framework…"
    mkdir -p "$ADAPTER_FRAMEWORK"
    clang -dynamiclib -fobjc-arc -fvisibility=default \
        -framework Foundation -framework AppKit -framework UniformTypeIdentifiers \
        -I "$ADAPTER_DIR/include" -I "$ADAPTER_DIR/src" \
        "$ADAPTER_DIR"/src/adapter/*.m "$ADAPTER_DIR"/src/private/*.m \
        "$ADAPTER_DIR"/src/utility/*.m \
        -o "$ADAPTER_FRAMEWORK/MediaRemoteAdapter"
    codesign --force --sign - "$ADAPTER_FRAMEWORK"
fi

rm -rf "$BUNDLE"
mkdir -p "$BUNDLE/Contents/MacOS" "$BUNDLE/Contents/Frameworks" "$BUNDLE/Contents/Resources"
cp "$BIN_DIR/$APP_NAME" "$BUNDLE/Contents/MacOS/$APP_NAME"
cp Resources/Info.plist "$BUNDLE/Contents/Info.plist"
ditto "$BIN_DIR/whisper.framework" "$BUNDLE/Contents/Frameworks/whisper.framework"

RES_DIR="$BUNDLE/Contents/Resources/mediaremote-adapter"
mkdir -p "$RES_DIR"
cp "$ADAPTER_DIR/bin/mediaremote-adapter.pl" "$RES_DIR/"
cp -R "$ADAPTER_FRAMEWORK" "$RES_DIR/"

IDENTITIES="$(security find-identity -v -p codesigning 2>/dev/null)"
APPLE_DEVELOPMENT="$(awk '/"Apple Development: /{print $2; exit}' <<<"$IDENTITIES")"
if [[ -n "$APPLE_DEVELOPMENT" ]]; then
    IDENTITY="$APPLE_DEVELOPMENT"
elif grep -q "$SIGNING_IDENTITY_NAME" <<<"$IDENTITIES"; then
    IDENTITY="$SIGNING_IDENTITY_NAME"
    echo "note: signing with '$SIGNING_IDENTITY_NAME'; the Keychain will ask again after each rebuild (see README: Permissions)" >&2
else
    IDENTITY="-"
    echo "note: no signing certificate, signing ad-hoc (see README: Permissions)" >&2
fi
codesign --force --sign "$IDENTITY" "$BUNDLE/Contents/Frameworks/whisper.framework"
codesign --force --sign "$IDENTITY" "$BUNDLE"
echo "Built $BUNDLE"

if [[ "${1:-}" == "--dist" ]]; then
    rm -f "$DIST_DIR/$APP_NAME.zip"
    ditto -c -k --keepParent "$BUNDLE" "$DIST_DIR/$APP_NAME.zip"
    echo "Packaged $DIST_DIR/$APP_NAME.zip"
    exit 0
fi

pkill -x "$APP_NAME" 2>/dev/null || true
sleep 0.3
rm -rf "$INSTALL_DIR"
ditto "$BUNDLE" "$INSTALL_DIR"
echo "Installed $INSTALL_DIR"

if [[ "${1:-}" == "--run" ]]; then
    open "$INSTALL_DIR"
    echo "Launched $APP_NAME"
fi
