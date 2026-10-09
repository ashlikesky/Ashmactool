#!/bin/bash
set -euo pipefail
project_root="$(cd "$(dirname "$0")/.." && pwd)"
output_root="${ASHMACTOOL_OUTPUT_DIR:-$(dirname "$project_root")}"
app_bundle="$output_root/Ashmactool.app"
cd "$project_root"
export MACOSX_DEPLOYMENT_TARGET=14.0
cargo build --release --locked
build_root="${CARGO_TARGET_DIR:-$project_root/target}"
binary="$build_root/release/ashmactool"
icon_work="$(mktemp -d "${TMPDIR:-/tmp}/ashmactool-icon.XXXXXX")"
trap 'rm -rf "$icon_work"' EXIT
iconset="$icon_work/AppIcon.iconset"
mkdir -p "$iconset" "$app_bundle/Contents/MacOS" "$app_bundle/Contents/Resources"
for size in 16 32 128 256 512; do
    sips -z "$size" "$size" assets/AppIcon.png --out "$iconset/icon_${size}x${size}.png" >/dev/null
    double=$((size * 2))
    sips -z "$double" "$double" assets/AppIcon.png --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$iconset" -o "$app_bundle/Contents/Resources/AppIcon.icns"
cp "$binary" "$app_bundle/Contents/MacOS/ashmactool"
"$binary" --preview previews
cp LICENSE THIRD_PARTY_NOTICES.md "$app_bundle/Contents/Resources/"
cp -R licenses "$app_bundle/Contents/Resources/"
cat > "$app_bundle/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>com.ashlikesky.ashmactool</string>
<key>CFBundleName</key><string>Ashmactool</string>
<key>CFBundleDisplayName</key><string>Ashmactool</string>
<key>CFBundleExecutable</key><string>ashmactool</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>0.3.1</string>
<key>CFBundleVersion</key><string>4</string>
<key>CFBundleIconFile</key><string>AppIcon.icns</string>
<key>LSMinimumSystemVersion</key><string>14.0</string>
<key>LSUIElement</key><true/>
<key>NSHighResolutionCapable</key><true/>
<key>NSHumanReadableCopyright</key><string>Copyright 2026 Ashmactool contributors. MIT license. Cursor API origins: Mousecape / CGSInternal. See bundled notices.</string>
</dict></plist>
PLIST
codesign --force --sign - --identifier com.ashlikesky.ashmactool --timestamp=none "$app_bundle"
codesign --verify --deep --strict "$app_bundle"
ditto -c -k --sequesterRsrc --keepParent "$app_bundle" "$icon_work/Ashmactool.zip"
mv "$icon_work/Ashmactool.zip" "$output_root/Ashmactool-macOS-$(uname -m).zip"
printf 'Built: %s\n' "$app_bundle"
du -sh "$app_bundle" "$output_root/Ashmactool-macOS-$(uname -m).zip"
