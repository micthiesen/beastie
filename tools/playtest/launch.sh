#!/bin/zsh
# Restart exactly one native game instance on a fresh save and wait for its window.
S=${BEASTIE_PLAYTEST_DIR:-/tmp/beastie-playtest}
HERE=${0:A:h}
pkill -f "dev-perf/beastie-game"; pkill -f "xtask dev"; sleep 1
cd /Users/michael/Code/beastie
flags=(--fake-ai); [ -n "$BEASTIE_REAL" ] && flags=()
(cargo xtask dev "${flags[@]}" --new-game "$@" > $S/dev.log 2>&1 &)
for i in $(seq 1 150); do
  sleep 2
  osascript -e 'tell application "System Events" to get name of every process whose name contains "beastie"' 2>/dev/null | grep -q beastie && break
done
sleep 4
osascript -e 'tell application "System Events" to set frontmost of (first process whose name contains "beastie") to true'
rm -f $S/geom.cache; $HERE/g.sh geom
