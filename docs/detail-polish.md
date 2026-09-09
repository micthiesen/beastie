# Aquarium detail polish

Date: 2026-09-08. Status: implemented and independently reviewed; native comparison complete.

The user authorized a holistic review and all worthwhile improvements in this session, with no
further input. This contract records the concrete detail work discovered by that review. Preserve
the existing composition, creature identity, simulation authority, offline operation and optional
workers. No new gameplay or save migration is required.

## Accepted scope

1. Separate input-binding controls into readable rows. Leave a clear gap below Back and between
   focus outlines. Preserve Normal/Large text, all six actions, keyboard order and rebinding.
2. Replace the water backdrop's discrete constant-color slabs with a continuous geometric color
   gradient. Preserve its extent and subdued palette without animated noise or an extra image pass.
3. Refine area-light visibility to reduce the visibly separated shadow lobes beneath suspended
   toys. Compare the same scene and retain the change only if it improves contact/depth without
   distracting noise, movement instability or an unreasonable measured frame-cost increase.
4. Evaluate silhouette coverage, especially thin world details and UI edges. Preserve immediate
   response and clear glyphs; no temporal accumulation, ghosting or optional GPU capability.
5. Restore focus to the originating binding row after accepting or cancelling key capture.
   Native Food-to-F6 input exposed an unwanted jump to Push to talk. Preserve key swapping,
   Escape cancellation and navigation order; cover both return paths with regression tests.
6. Make canonical feel captures reject blank frames and isolate scripted input from live devices.
   The first final run exposed both acceptance gaps: live pointer actions changed causality and
   black video still passed format/hash checks. A capture-only flag must leave ordinary play and
   explicit native-input recordings unchanged. Closing the window must remain possible. Validate
   the guards with headless tests and reject the contaminated run explicitly.

Review other existing feel areas throughout. Add any further accepted material finding here before
implementation. Reject speculative effects that compete with the creature or lack valid evidence.

## Selected presentation

The backdrop is one closed cuboid spanning the old slab union, with endpoint colors converted
to linear space and interpolated at its vertices. It removes 300 authored triangles and keeps
the existing aquarium extent and unlit background material.

The light is a fixed disk perpendicular to the key direction, radius 0.20. Six interleaved
directions resolve ordinary surfaces. Upward rough receivers (roughness above 0.9, normal Y above
0.95) refine mixed visibility to twelve directions. Uniform visibility and other materials retain
six. This is bounded spatial quadrature, not a temporal denoiser or physical glass simulation.
Thin penumbrae that miss the first six directions remain a sampling limitation. Two world/four UI
coverage samples remain as before: uniform and adaptive four-ray world experiments cost too much.

Binding controls retain 91x14 hit regions at a 17-unit row pitch. The panel extends upward and
leaves the status strip clear. Returning from successful key entry or Escape resolves the originating
row by its semantic action in the current plan. Key swapping and persistence rules are unchanged;
no model, audio, authoritative state or save-format change is required.

## Validation and completion

Keep same-seed native before/after evidence. Review all Normal/Large surfaces and actual affected
pointer/keyboard input. Run the ordinary `cargo xtask dev --fake-ai` path, focused regression tests,
shader translation checks and `cargo xtask verify`. Complete two valid final native review passes
after the last material change. Independent visual, causal/audio and code review findings must be
adjudicated. Record evidence limits, cost measurements and rejected experiments in
`docs/feel-review-details-20260908.md`, update STATE, commit and push to main. Do not rebuild release
packages or model bundles. Human full-speed perception, listening and other hardware remain
unclaimed unless actually available.
