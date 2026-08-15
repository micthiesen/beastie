#!/usr/bin/env bash
set -euo pipefail

# Prepare deterministic Steam content and optionally invoke steamcmd. This script never invents
# IDs: release mode must receive validated values from a private environment/config file.
mode=developer
package_root=
output=${STEAM_BUILD_OUTPUT:-dist/steam}
branch=${STEAM_BRANCH:-default}
config=
force=false
while (($#)); do
  case "$1" in
    --mode) mode=${2:?missing value for --mode}; shift 2 ;;
    --package-root) package_root=${2:?missing value for --package-root}; shift 2 ;;
    --output) output=${2:?missing value for --output}; shift 2 ;;
    --branch) branch=${2:?missing value for --branch}; shift 2 ;;
    --config) config=${2:?missing value for --config}; shift 2 ;;
    --force) force=true; shift ;;
    *) echo "usage: $0 --package-root <dist> [--mode developer|release] [--output dir] [--branch name] [--config path] [--force]" >&2; exit 2 ;;
  esac
done
[[ -n "$package_root" ]] || { echo '--package-root is required' >&2; exit 2; }
validation_args=(--mode "$mode")
if [[ -n "$config" ]]; then
  validation_args+=(--config "$config")
fi
bash steam/validate-config.sh "${validation_args[@]}"
if [[ -n "$config" ]]; then
  set -a
  # shellcheck disable=SC1090
  source "$config"
  set +a
fi
for platform in windows macos linux; do
  [[ -f "$package_root/$platform/package-manifest.json" ]] || { echo "missing $platform package" >&2; exit 1; }
done
if [[ -e "$output" || -L "$output" ]]; then
  [[ "$force" == true ]] || {
    echo "output already exists: $output (pass --force to replace it)" >&2
    exit 1
  }
  rm -rf -- "$output"
fi
mkdir -p "$output"
cp -R steam/achievements.json steam/input "$output/"
export STEAM_CONTENT_ROOT="$package_root"
export STEAM_BUILD_OUTPUT="$output"
export STEAM_BRANCH="$branch"
export STEAM_PREVIEW=${STEAM_PREVIEW:-1}
for template in steam/steam_app_build.vdf.in steam/depot_windows.vdf steam/depot_macos.vdf steam/depot_linux.vdf; do
  target="$output/$(basename "$template" .in)"
  python3 steam/render-vdf.py "$template" "$target"
done
echo "Steam content prepared at $output (preview=$STEAM_PREVIEW)"
if [[ "$mode" == release ]]; then
  [[ -x "${STEAMCMD}" ]] || { echo "STEAMCMD is not executable: ${STEAMCMD}" >&2; exit 1; }
  "$STEAMCMD" +login "${STEAM_USERNAME:?STEAM_USERNAME required}" +run_app_build "$output/steam_app_build.vdf" +quit
fi
