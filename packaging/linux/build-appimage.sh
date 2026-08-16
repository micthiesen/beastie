#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage: $0 --package-root <dist/linux> --output <Beastie.AppImage|Beastie.tar.gz> [--format appimage|tar.gz] [--force]" >&2
  exit 2
}

package_root=
output=
format=auto
force=false
while (($#)); do
  case "$1" in
    --package-root) package_root=${2:?missing value for --package-root}; shift 2 ;;
    --output) output=${2:?missing value for --output}; shift 2 ;;
    --format) format=${2:?missing value for --format}; shift 2 ;;
    --force) force=true; shift ;;
    *) usage ;;
  esac
done
[[ -n "$package_root" && -n "$output" ]] || usage
[[ -f "$package_root/package-manifest.json" ]] || { echo "missing verified package manifest: $package_root" >&2; exit 1; }
[[ -x "$package_root/beastie" ]] || { echo "packaged game is not executable: $package_root/beastie" >&2; exit 1; }

if [[ "$format" == auto ]]; then
  if command -v appimagetool >/dev/null 2>&1; then format=appimage; else format=tar.gz; fi
fi
case "$format" in appimage|tar.gz) ;; *) echo "format must be appimage or tar.gz" >&2; exit 2 ;; esac
if [[ -e "$output" || -L "$output" ]]; then
  [[ "$force" == true ]] || {
    echo "output already exists: $output (pass --force to replace it)" >&2
    exit 1
  }
  rm -f -- "$output"
fi

staging=$(mktemp -d "${TMPDIR:-/tmp}/beastie-appimage.XXXXXX")
trap 'rm -rf "$staging"' EXIT
appdir=$staging/Beastie.AppDir
mkdir -p "$appdir/usr/bin"
cp -R "$package_root"/. "$appdir/usr/bin/"
cat > "$appdir/AppRun" <<'APPRUN'
#!/usr/bin/env bash
set -euo pipefail
here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
cd "$here/usr/bin"
exec "$here/usr/bin/beastie" "$@"
APPRUN
chmod +x "$appdir/AppRun"
cat > "$appdir/beastie.desktop" <<'DESKTOP'
[Desktop Entry]
Type=Application
Name=Beastie
Exec=AppRun
Icon=beastie
Categories=Game;
DESKTOP
cat > "$appdir/README-LINUX.txt" <<'README'
Beastie for Linux x64

Extract the archive and run:

    ./AppRun

Beastie runs entirely offline and stores saves and generated speech in your
normal per-user XDG configuration directory, never beside the executable.

This tar bundle targets an x86-64 Linux desktop with X11 or Wayland, Vulkan or
Mesa graphics, ALSA, udev, and xkbcommon-x11. Ubuntu 24.04 Desktop includes
these libraries. A minimal Ubuntu installation can add them with:

    sudo apt install libxkbcommon-x11-0 libasound2t64 libudev1 mesa-vulkan-drivers
README
mkdir -p "$(dirname "$output")"
if [[ "$format" == appimage ]]; then
  appimagetool "$appdir" "$output"
else
  tar -czf "$output" -C "$appdir" .
fi
echo "created $output ($format)"
