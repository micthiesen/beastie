#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage: $0 --app <Beastie.app> --output <Beastie.dmg> [--volume-name <name>] [--force]" >&2
  exit 2
}

app=
output=
volume_name=Beastie
force=false
while (($#)); do
  case "$1" in
    --app) app=${2:?missing value for --app}; shift 2 ;;
    --output) output=${2:?missing value for --output}; shift 2 ;;
    --volume-name) volume_name=${2:?missing value for --volume-name}; shift 2 ;;
    --force) force=true; shift ;;
    *) usage ;;
  esac
done
[[ -n "$app" && -n "$output" ]] || usage
[[ "$(uname -s)" == Darwin ]] || { echo "DMG assembly requires macOS (use CI macos-latest)" >&2; exit 1; }
[[ -d "$app/Contents/MacOS" ]] || { echo "not a Beastie app bundle: $app" >&2; exit 1; }
codesign --verify --deep --strict "$app"

if [[ -e "$output" || -L "$output" ]]; then
  [[ "$force" == true ]] || {
    echo "output already exists: $output (pass --force to replace it)" >&2
    exit 1
  }
  rm -f -- "$output"
fi

staging=$(mktemp -d "${TMPDIR:-/tmp}/beastie-dmg.XXXXXX")
trap 'rm -rf "$staging"' EXIT
cp -R "$app" "$staging/"
ln -s /Applications "$staging/Applications"
mkdir -p "$(dirname "$output")"
hdiutil create -volname "$volume_name" -srcfolder "$staging" -format UDZO -imagekey zlib-level=9 -ov "$output"
hdiutil verify "$output" >/dev/null
echo "created $output"
