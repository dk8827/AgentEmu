#!/usr/bin/env sh
# Local ad-hoc signed development app. Developer ID/notarization is a release task.
set -eu
cd "$(dirname "$0")/.."
if [ "$(uname -s)" != Darwin ]; then
  echo "This app bundle requires macOS." >&2
  exit 1
fi
. ./scripts/env.sh
cargo build --locked --release -p frontend-native
bundle="target/dist/AgentEmu.app"
mkdir -p "$bundle/Contents/MacOS" "$bundle/Contents/Resources/licenses"
cp LICENSE THIRD_PARTY_NOTICES.md third-party/rust-notices.txt "$bundle/Contents/Resources/licenses/"
cp crates/player-ui/assets/OFL.txt crates/nes-video/palettes/LICENSE-MIT-0 "$bundle/Contents/Resources/licenses/"
cp crates/player-ui/assets/gallery/NOTICE.md "$bundle/Contents/Resources/licenses/GALLERY-NOTICE.md"
# Bundled model weights use the same MIT terms; retain their individual notices.
for model in models/*; do
  mkdir -p "$bundle/Contents/Resources/licenses/$(basename "$model")"
  cp "$model/LICENSE" "$bundle/Contents/Resources/licenses/$(basename "$model")/LICENSE"
done
cp target/release/agentemu "$bundle/Contents/MacOS/AgentEmu"
cat > "$bundle/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleExecutable</key><string>AgentEmu</string>
  <key>CFBundleIdentifier</key><string>dev.agentemu.player</string>
  <key>CFBundleName</key><string>AgentEmu</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleVersion</key><string>0.1.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict></plist>
PLIST
# Replacing a previously signed executable invalidates the old bundle seal.
# Refresh the local ad-hoc signature so macOS can launch every rebuilt bundle.
/usr/bin/codesign --force --sign - "$bundle"
/usr/bin/codesign --verify --strict "$bundle"
printf 'Built %s/%s\n' "$PWD" "$bundle"
