#!/usr/bin/env bash
set -euo pipefail

# Build a sealed, notarization-ready .app from the already verified package layout.
# The executable stays beside its worker/runtime siblings in Contents/MacOS because Beastie
# discovers the offline bundle relative to current_exe().

usage() {
  echo "usage: $0 --package-root <dist/macos> --output <Beastie.app> [--version <version>] [--sign <identity>] [--force]" >&2
  exit 2
}

package_root=
output=
version=1.0.0
sign_identity=
force=false
while (($#)); do
  case "$1" in
    --package-root) package_root=${2:?missing value for --package-root}; shift 2 ;;
    --output) output=${2:?missing value for --output}; shift 2 ;;
    --version) version=${2:?missing value for --version}; shift 2 ;;
    --sign) sign_identity=${2:?missing value for --sign}; shift 2 ;;
    --force) force=true; shift ;;
    *) usage ;;
  esac
done

[[ -n "$package_root" && -n "$output" ]] || usage
[[ "$(uname -s)" == Darwin ]] || { echo "macOS .app assembly requires macOS (use CI macos-latest)" >&2; exit 1; }
[[ -f "$package_root/package-manifest.json" ]] || { echo "missing verified package manifest: $package_root" >&2; exit 1; }
[[ -x "$package_root/beastie" ]] || { echo "packaged game is not executable: $package_root/beastie" >&2; exit 1; }

app_name=$(basename "$output")
[[ "$app_name" == *.app ]] || { echo "output must end in .app" >&2; exit 1; }
app_root=$output
contents=$app_root/Contents
macos=$contents/MacOS
resources=$contents/Resources
if [[ -e "$app_root" || -L "$app_root" ]]; then
  [[ "$force" == true ]] || {
    echo "output already exists: $app_root (pass --force to replace it)" >&2
    exit 1
  }
  rm -rf -- "$app_root"
fi
mkdir -p "$macos" "$resources"
cp -R "$package_root"/. "$macos"/

cat > "$contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key><string>en</string>
  <key>CFBundleExecutable</key><string>beastie</string>
  <key>CFBundleIdentifier</key><string>com.beastie.game</string>
  <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
  <key>CFBundleName</key><string>Beastie</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleVersion</key><string>$version</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><false/>
  <key>NSMicrophoneUsageDescription</key>
  <string>Beastie uses the microphone only while you hold push-to-talk, for local speech recognition.</string>
</dict>
</plist>
PLIST
plutil -lint "$contents/Info.plist" >/dev/null

# Remove quarantine metadata copied from downloaded inputs. This does not sign the app.
xattr -dr com.apple.quarantine "$app_root" 2>/dev/null || true
if [[ -n "$sign_identity" ]]; then
  codesign --deep --force --options runtime --timestamp --sign "$sign_identity" "$app_root"
else
  codesign --deep --force --options runtime --sign - "$app_root"
  echo "ad hoc signed app assembled (pass --sign with a validated Developer ID for release signing)"
fi
codesign --verify --deep --strict "$app_root"

echo "created $app_root"
