#!/usr/bin/env bash
set -euo pipefail

mode=developer
config=
while (($#)); do
  case "$1" in
    --mode) mode=${2:?missing value for --mode}; shift 2 ;;
    --config) config=${2:?missing value for --config}; shift 2 ;;
    *) echo "usage: $0 [--mode developer|release] [--config path]" >&2; exit 2 ;;
  esac
done
[[ "$mode" == developer || "$mode" == release ]] || { echo 'mode must be developer or release' >&2; exit 2; }
if [[ -n "$config" ]]; then
  # shellcheck disable=SC1090
  source "$config"
fi

if [[ "$mode" == developer ]]; then
  echo 'developer mode: Steam IDs remain placeholders; publishing is disabled'
  exit 0
fi

for variable in STEAM_APP_ID STEAM_DEPOT_WINDOWS_ID STEAM_DEPOT_MACOS_ID STEAM_DEPOT_LINUX_ID; do
  value=${!variable:-}
  if [[ ! "$value" =~ ^[1-9][0-9]{2,11}$ ]]; then
    echo "$variable must be a real numeric Steam ID in release mode (placeholder rejected)" >&2
    exit 1
  fi
done
[[ -n "${STEAMCMD:-}" && "${STEAMCMD}" != steamcmd ]] || {
  echo 'STEAMCMD must point to the release steamcmd executable in release mode' >&2
  exit 1
}
echo "Steam release configuration validated for app $STEAM_APP_ID"
