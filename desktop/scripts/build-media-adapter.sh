#!/bin/sh
set -eu
[ "$(uname)" = "Darwin" ] || exit 0
cd "$(dirname "$0")/.."

ADAPTER_DIR="../vendor/mediaremote-adapter"
OUT_DIR="src-tauri/resources/mediaremote-adapter"
FRAMEWORK="$OUT_DIR/MediaRemoteAdapter.framework"

mkdir -p "$FRAMEWORK"
cp "$ADAPTER_DIR/bin/mediaremote-adapter.pl" "$OUT_DIR/"
if [ ! -f "$FRAMEWORK/MediaRemoteAdapter" ]; then
    clang -dynamiclib -fobjc-arc -fvisibility=default \
        -framework Foundation -framework AppKit -framework UniformTypeIdentifiers \
        -I "$ADAPTER_DIR/include" -I "$ADAPTER_DIR/src" \
        "$ADAPTER_DIR"/src/adapter/*.m "$ADAPTER_DIR"/src/private/*.m \
        "$ADAPTER_DIR"/src/utility/*.m \
        -o "$FRAMEWORK/MediaRemoteAdapter"
    codesign --force --sign - "$FRAMEWORK"
fi
