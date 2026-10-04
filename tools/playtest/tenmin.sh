#!/bin/zsh
# A ten-minute fresh-save playthrough with real OS input, following the game's own hints the way
# a newcomer would. Logs actions with wall time; screenshots every ~2 s; the game writes its own
# event trace (BEASTIE_EVENT_LOG) alongside.
S=${BEASTIE_PLAYTEST_DIR:-/tmp/beastie-playtest}
HERE=${0:A:h}
OUT=$S/ten-$1; rm -rf $OUT; mkdir -p $OUT
G=$HERE/g.sh
T0=$(python3 -c 'import time;print(time.time())')
log() { python3 -c "import time;print(f'{time.time()-$T0:7.2f} $1')" >> $OUT/actions.log; }
shots() { i=0; while [ -f $OUT/.running ]; do $G shot ten-$1/f$(printf %04d $i) 2>/dev/null; i=$((i+1)); sleep 1.8; done; }
say() { log "say: $1"; $G click 230 164; $G type "$1"; $G key return; }
pet() { $G shot ten-$1/probe; pos=$(python3 $HERE/findmop.py $OUT/probe.png) && { log "pet at $pos"; $G click ${=pos}; } || log "pet: Mop not found"; }
wait_s() { sleep $1; }
echo "$T0" > $OUT/t0; touch $OUT/.running; shots $1 &
log "meet"; $G click 160 88; wait_s 6
log "tap glass"; $G click 240 50; wait_s 4
log "play ball"; $G click 66 164; wait_s 1.5
say "ball"; wait_s 2.5; say "ball"; wait_s 5
say "ball"; wait_s 8
log "idle watch"; wait_s 18
log "feed berry"; $G click 130 164; wait_s 1.2; say "berry"; wait_s 2; say "yummy berry"; wait_s 8
pet $1; wait_s 1.2; say "mop"; wait_s 3; say "good mop"; wait_s 6
log "play sock"; $G click 106 164; wait_s 1.2; say "sock"; wait_s 2.5; say "sock"; wait_s 8
say "ball"; wait_s 10
log "idle watch"; wait_s 25
log "play bell"; $G click 86 164; wait_s 1.2; say "ding"; wait_s 2.5; say "ding"; wait_s 8
say "mop"; wait_s 6
log "feed mushroom"; $G click 150 164; wait_s 6
say "no"; wait_s 6
pet $1; wait_s 2; say "good"; wait_s 6
say "ding"; wait_s 10
log "idle watch"; wait_s 30
say "berry"; wait_s 6
log "feed berry"; $G click 130 164; wait_s 6
say "sock"; wait_s 10
log "tap glass"; $G click 60 60; wait_s 5
log "idle watch"; wait_s 40
pet $1; wait_s 3; say "mop"; wait_s 8
log "play ball"; $G click 66 164; wait_s 1.5; say "ball"; wait_s 10
log "idle watch"; wait_s 40
say "come"; wait_s 4; say "come"; wait_s 8
log "feed pellet"; $G click 170 164; wait_s 2; say "pellet"; wait_s 2; say "pellet"; wait_s 8
log "idle watch"; wait_s 60
say "mop"; wait_s 8
log "idle watch"; wait_s 60
rm $OUT/.running; wait
log "end"
