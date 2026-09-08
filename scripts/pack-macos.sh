#!/usr/bin/env bash
set -euo pipefail

export COPYFILE_DISABLE=1

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
APP_NAME="Voxel Editor.app"
APP="$ROOT/dist/$APP_NAME"
TMP_APP="${TMPDIR:-/tmp}/$APP_NAME"
BIN="$ROOT/target/release/voxel-editor"
MCP_BIN="$ROOT/target/release/voxel-mcp-server"
ICNS="$ROOT/assets/icons/macos/AppIcon.icns"
PLIST="$ROOT/assets/macos/Info.plist"
PROJECT="${HOME}/Documents/n2d98 Voxel Editor/project.vox"

strip_appledouble() {
  local root="$1"
  find "$root" -name '._*' -delete
  xattr -cr "$root" >/dev/null 2>&1 || true
}

if [[ ! -f "$ICNS" ]]; then
  python3 "$ROOT/scripts/generate-icons.py"
fi

# Cursor agent shells set CARGO_TARGET_DIR to a sandbox cache; pack must
# write the repo's target/release so the .app and MCP configs stay in sync.
(cd "$ROOT" && env -u CARGO_TARGET_DIR cargo build --release -p voxel_editor -p voxel_mcp_server)

rm -rf "$TMP_APP" "$APP"
mkdir -p "$TMP_APP/Contents/MacOS" "$TMP_APP/Contents/Resources"
cp -X "$BIN" "$TMP_APP/Contents/MacOS/voxel-editor"
cp -X "$MCP_BIN" "$TMP_APP/Contents/MacOS/voxel-mcp-server"
chmod +x "$TMP_APP/Contents/MacOS/voxel-editor" "$TMP_APP/Contents/MacOS/voxel-mcp-server"
cp -X "$ICNS" "$TMP_APP/Contents/Resources/AppIcon.icns"
cp -X "$PLIST" "$TMP_APP/Contents/Info.plist"
printf 'APPL????' > "$TMP_APP/Contents/PkgInfo"
strip_appledouble "$TMP_APP"

if command -v codesign >/dev/null 2>&1; then
  codesign --force --deep --sign - "$TMP_APP"
fi

mkdir -p "$ROOT/dist"
ditto --norsrc --noextattr "$TMP_APP" "$APP"
strip_appledouble "$APP"
rm -rf "$TMP_APP"

python3 - "$APP/Contents/MacOS/voxel-mcp-server" "$PROJECT" "$ROOT/dist/mcp.json" <<'PY'
import json, sys
command, project, dest = sys.argv[1], sys.argv[2], sys.argv[3]
cfg = {
    "mcpServers": {
        "voxel-editor": {
            "command": command,
            "args": ["--project", project, "--size", "32"],
            "env": {"RUST_LOG": "info"},
        }
    }
}
with open(dest, "w") as f:
    json.dump(cfg, f, indent=2)
    f.write("\n")
print(f"Wrote {dest}")
PY

echo "Packed $APP"
echo "MCP project: $PROJECT"
ls -lh "$APP/Contents/MacOS/voxel-editor" "$APP/Contents/MacOS/voxel-mcp-server" "$APP/Contents/Resources/AppIcon.icns"
