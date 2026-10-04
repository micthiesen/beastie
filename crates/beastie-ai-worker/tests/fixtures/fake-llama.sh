#!/bin/sh
set -eu

mode=valid
state=
record=
while [ "$#" -gt 0 ]; do
  case "$1" in
    --fake-mode) mode=$2; shift 2 ;;
    --fake-state) state=$2; shift 2 ;;
    --fake-record) record=$2; shift 2 ;;
    *)
      if [ -n "$record" ]; then
        printf '%s\n' "$1" >> "$record"
      fi
      shift
      ;;
  esac
done

reply='want berry! berry!'
case "$mode" in
  valid) printf '%s\n' "$reply" ;;
  retry)
    if [ ! -e "$state" ]; then
      : > "$state"
      printf '%s\n' 'hello friend'
    else
      printf '%s\n' "$reply"
    fi
    ;;
  timeout) while :; do :; done ;;
  oversized) awk 'BEGIN { for (i = 0; i < 4096; i++) printf "x" }' ;;
  unknown) printf '%s\n' 'I would love a berry please' ;;
  *) exit 2 ;;
esac
