# Live-input playtest scripts (macOS)

Drive the native game with real OS input to time a fresh-save session, as used in
[the fun feel review](../../docs/feel-review-fun-20261004.md). Needs `cliclick`, Pillow and
macOS Accessibility permission for the terminal. Output goes to `$BEASTIE_PLAYTEST_DIR`
(default `/tmp/beastie-playtest`).

```sh
mkdir -p /tmp/beastie-playtest
export BEASTIE_EVENT_LOG=/tmp/beastie-playtest/events-1.jsonl   # opt-in game event trace
tools/playtest/launch.sh                  # rebuild and start one fresh game (composer voice)
tools/playtest/tenmin.sh 1                # ten-minute scripted newcomer, screenshots every 1.8 s
python3 tools/playtest/score.py /tmp/beastie-playtest/ten-1 /tmp/beastie-playtest/events-1.jsonl
```

For the real local model, set `BEASTIE_REAL=1 BEASTIE_AI_BACKEND=llama-server
BEASTIE_AI_MODEL=models/Qwen3.5-0.8B-Q4_0.gguf BEASTIE_LLAMA_SERVER=$(which llama-server)` before
`launch.sh`. `g.sh` maps the 320×180 UI grid onto the window; `findmop.py` locates Mop by color
so the script can pet it. Times are wall-clock and include a few milliseconds of driver latency.
