---
name: wrap
description: >
  Conclude a Beastie work session so a fresh Codex session can continue seamlessly: encode durable
  learnings in the right repository homes, decide or confirm the next step, rewrite docs/STATE.md,
  run the gate, commit, and push. Trigger on "wrap up", "conclude the session", "save state", or
  /wrap.
user_invocable: true
---

# Wrap (Conclude The Session)

After a wrap, the current session is disposable: durable knowledge is in the repository and the
next step is recorded with its reason.

## 1. Sweep Learnings Into Their Homes

Review the session for anything durable that is not encoded yet:

| Learning | Home |
|---|---|
| Product or architecture decision | relevant `docs/*.md` |
| Deterministic scenario or regression | `fixtures/` plus a test |
| Prompt/dialogue behavior | versioned prompt source or `evals/dialogue/` |
| Model/runtime benchmark or license | `models/manifest.toml` and a focused doc |
| Asset prompt, seed, provenance, or override | `assets/manifest.toml` |
| Repeatable agent procedure | matching `.claude/skills/` skill |
| Cross-cutting convention or invariant | `AGENTS.md` |

Write the content in its real home. `docs/STATE.md` gets only pointers and decisions. Record dead
ends too, because they prevent the next session from repeating failed work.

## 2. Decide Or Confirm Next

- Carry the previous Next forward if it remains the plan and is incomplete.
- Run `/next` if the previous Next landed or the session changed the dependency picture.

Do not improvise a major pivot inside the wrap.

## 3. Rewrite STATE.md

Rewrite `docs/STATE.md` wholesale, never append. Keep only:

- **Now**: current work picture in 3-6 bullets;
- **Next**: the chosen step and why;
- **Candidates Not Chosen**: still-relevant runners-up;
- **Learned Recently**: short pointers to durable homes;
- current Last updated date.

Do not add machine, worker, or uncommitted-git snapshots.

## 4. Verify And Ship

Run `cargo xtask verify`. For visible shell changes, also run `cargo xtask dev --fake-ai`. Fix any
failure, then commit all finished project changes and push directly to `main` per `AGENTS.md`.

## 5. Sign Off

Report that learnings are encoded, STATE is current, verification passed, and the commit was
pushed. Name the recorded Next in one line.
