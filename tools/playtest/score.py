#!/usr/bin/env python3
"""Score a ten-minute playthrough from its action log and the game's event trace."""
import json, sys
run, trace = sys.argv[1], sys.argv[2]
t0 = float(open(f"{run}/t0").read()) * 1000
acts = []
for line in open(f"{run}/actions.log"):
    t, what = line.strip().split(None, 1)
    acts.append((float(t), what))
recs = [json.loads(l) for l in open(trace)]
def at(r): return (r["wall_ms"] - t0) / 1000
events = [(at(r), e) for r in recs if "events" in r for e in r["events"]]
says = [(at(r), r) for r in recs if "say" in r]
first_input = acts[1][0] if len(acts) > 1 else None
reacts = [t for t, e in events if t >= first_input - 0.05]
print(f"first input {first_input:.1f}s -> first event {reacts[0]-first_input:.2f}s later")
learned = [(t, e["value"]["word"]) for t, e in events if e["kind"] == "word_learned"]
print("words learned:", [(round(t, 1), w) for t, w in learned])
if learned:
    w0 = learned[0][1]
    used = [t for t, r in says if t > learned[0][0] and w0 in r["say"].lower().split() or (t > learned[0][0] and w0 in r["say"].lower())]
    print(f"first word {w0!r} at {learned[0][0]:.1f}s; Mop uses it at {used[0]:.1f}s" if used else "never used")
payoffs = []
for t, what in acts:
    kind = None
    if what.startswith("play"): kind = ("toy_played", "toy_rejected")
    if what.startswith("feed"): kind = ("food_consumed", "food_rejected")
    if what.startswith("pet"): kind = ("comforted",)
    if kind:
        hit = [te for te, e in events if te >= t - 0.05 and e["kind"] in kind]
        payoffs.append((round(t, 1), what, round(hit[0] - t, 2) if hit else None))
print("payoffs (action, seconds to payoff):", payoffs)
lines = [r["say"] for t, r in says]
print(f"{len(lines)} lines, {len(set(lines))} distinct; fallback banners: {sum(1 for t,r in says if r.get('fallback') and r.get('fallback_reason') not in (None,'worker_unavailable'))}")
gaps = []
stamps = sorted([t for t, e in events if e["kind"] not in ("need_changed",)] + [t for t, _ in says])
for a, b in zip(stamps, stamps[1:]):
    if b - a > 20: gaps.append((round(a, 1), round(b - a, 1)))
print("quiet stretches over 20 s:", gaps)
