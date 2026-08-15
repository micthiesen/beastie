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

reply='{"protocol_version":1,"request_id":41,"say":"berry remains bad.","gesture":"look_player","recalled_memory":41}'
case "$mode" in
  valid) printf '%s\n' "$reply" ;;
  retry)
    if [ ! -e "$state" ]; then
      : > "$state"
      printf '%s\n' 'not json'
    else
      printf '%s\n' "$reply"
    fi
    ;;
  timeout) while :; do :; done ;;
  oversized) awk 'BEGIN { for (i = 0; i < 4096; i++) printf "x" }' ;;
  echo) printf '%s\n' '{"protocol_version":1,"request_id":41,"say":"Do you remember the berry?","gesture":"none","recalled_memory":null}' ;;
  profanity) printf '%s\n' '{"protocol_version":1,"request_id":41,"say":"Damn berry.","gesture":"none","recalled_memory":null}' ;;
  *) exit 2 ;;
esac
