#!/bin/zsh
# Helpers for driving the native Beastie window. Coordinates are logical 320x180 UI units.
S=${BEASTIE_PLAYTEST_DIR:-/tmp/beastie-playtest}
geom() { osascript -e 'tell application "System Events" to get {position, size} of first window of (first process whose name contains "beastie")' | tr -d ' '; }
cmd=$1; shift
if [ -f $S/geom.cache ] && [ "$cmd" != geom ]; then G=$(cat $S/geom.cache); else G=$(geom); echo $G > $S/geom.cache; fi; IFS=, read WX WY WW WH <<< "$G"
# window content: title bar ~28pt; UI maps 320x180 into content area
TB=28; CH=$((WH-TB))
lx() { echo $(( WX + $1 * WW / 320 )); }
ly() { echo $(( WY + TB + $1 * CH / 180 )); }
case $cmd in
  shot) screencapture -x -R$WX,$WY,$WW,$WH $S/$1.png ;;
  click) cliclick c:$(lx $1),$(ly $2) ;;
  move) cliclick m:$(lx $1),$(ly $2) ;;
  type) cliclick t:"$1" ;;
  key) case $1 in return) osascript -e "tell application \"System Events\" to key code 36";; *) cliclick kp:$1;; esac ;;
  enter) osascript -e "tell application \"System Events\" to key code 36" ;;
  geom) echo $G ;;
esac
