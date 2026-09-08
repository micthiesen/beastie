---
name: next
description: >
  Decide what Beastie should build next. Reads docs/STATE.md, docs/mvp-spec.md, architecture
  docs, and recent history; compares 2-4 candidates by what they unblock, effort, and risk;
  recommends one; and records the decision in docs/STATE.md. Trigger on "what's next", "what
  should we do next", "pick the next step", or /next.
---

# Next (Decide The Next Step)

Turn "what should we build now?" into a short, recorded decision. The output is a recommendation
with its reason and an updated `docs/STATE.md` so the decision survives the session.

## 1. Gather Ground Truth

- Read `docs/STATE.md` first. Its previous Next and unchosen candidates are the initial shortlist.
- Read the relevant acceptance criteria and dependency order in `docs/mvp-spec.md` plus any focused
  architecture/design doc linked from STATE.
- Read git history since STATE's last-updated date to learn what actually landed.
- Confirm a candidate is not already done with `git grep` or history before selecting it.

Do not reread the entire deep spec when a linked section answers the question. STATE exists to make
this decision cheap.

## 2. Enumerate 2-4 Candidates

For each candidate, state:

- the concrete, independently finishable capability;
- which MVP acceptance criterion or downstream work it unblocks;
- rough effort and the largest unknown;
- whether it is headless/fixture-testable or needs a real model, renderer, audio device, platform,
  PixelLab credential, or human judgment.

## 3. Recommend One

Choose in this order:

1. Advance the berry-memory vertical slice and current STATE Next.
2. De-risk an architectural or performance assumption before building on it.
3. Prefer pure-core and fixture-backed work until a real integration is the load-bearing unknown.
4. Run genuinely independent work in parallel when it shortens the critical path.

State the recommendation in two sentences: what to build and why it beats the runner-up. Use
an independent design review before recording a costly or contentious pivot.

## 4. Record It

Rewrite the relevant parts of `docs/STATE.md`:

- **Next**: the chosen capability, its two-line reason, and a doc/spec pointer;
- **Candidates Not Chosen**: runners-up and why each waits;
- **Now**: adjust only when the work picture actually changed.

Commit if this is a mid-session decision. Leave it for `/wrap` if the session is ending now.
