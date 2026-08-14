# The first claim that reads a sensor

`crates/sim-data/tests/path_claims.rs` measured 120 numbers in the guided path's prose and
every one of them was ground truth — a voltage, a temperature, a charge level read straight
off the engine. One sentence in the path is not about ground truth at all:

> The trip is a *probe* crossing 343.15 K — the two probes sit on corner cells, and the cell
> that is genuinely hottest is at 344.52 K when it fires, so **protection is late by 1.3 K**
> of somebody else's temperature.

`344.52` has been claimed since the slice that measured this step. `1.3` could not be: it is
the difference between a true temperature and a **sensor** reading, and nothing in the file
read a sensor. The claim's own note said so, and went further — it wrote down what would
have to be true for the sentence to be right, and said the harness could not settle it:

> the sentence is about the probe's own reading, and a probe crosses by landing at or just
> above the threshold, so it would have to sit in roughly [343.17, 343.27] K, an overshoot
> of 0.02 to 0.12 K in one step. That is plausible on a pack heating this fast. It is not
> established, and this harness cannot establish it.

This slice establishes it. **The probe crosses at t = 155.5 s reading 343.2458 K** — an
overshoot of 0.0958 K, inside the interval that was written down before anything could read
it. The registered prediction is what makes this a result rather than a measurement: the
number was constrained in advance, and the constraint held.

It also settles the reader's arithmetic, which is the part worth teaching. A reader
reconstructing 1.3 from the two numbers the sentence prints computes 344.52 − 343.15 = 1.37
and gets **1.4**, not 1.3 — because the threshold is not the reading. The probe does not
trip at 343.15; it trips at the first frame *above* it, and where that frame lands is a
property of how fast the pack is heating.

## What was built

One quantity, `t_gap_k_at`, and the plumbing under it:

* [`Row`] gained `sensed: Option<Sensed>` — what the BMS had **measured** as of the end of
  that step, beside everything else on the row, which is ground truth. `None` on a pack
  with no BMS, because such a pack has no sensors rather than sensors reading zero.
* `Sensed` carries the highest probe temperature and the frame's own `sampled_at_s`. Two
  fields, not a clone of `SensorFrame`: the current and charge channels would be fields
  with no consulting code, which is the shape this file rejects everywhere else. The
  frame's *time* is carried whether or not a claim reads it, because it is what tells a
  measurement from a stale one.
* `t_gap_k_at` is **belief minus truth** — the hottest probe minus the hottest cell — which
  is the same subtraction, in the same order, as `soc_gap_pts_at` one channel up and as the
  page's own BMS panel: `fmtSigned(probeMax - t.t_max, 2, "K")`.

The value is therefore negative, and the sentence prints its magnitude with the sign in the
word *late*. That is `States::Magnitude`, which already existed and is fenced to negative
values — so the slice needed no new machinery on the claim side at all. The third claim in
the file to use it.

## Which frame, and why the number cannot say

Another step's prose states the mechanism: **protection decides from sensors sampled at the
end of the previous step.** So "the gap when it fires" has three readings, not one, and all
three were measured before the claim was written:

| reading | value |
| --- | --- |
| both at the trip row (156.0 s) | **1.31397** ← claimed |
| truth at the trip, the frame that caused it (155.5 s) | 1.27435 |
| both at the deciding frame (155.5 s) | 1.30297 |

**All three print 1.3.** The spread between them is 0.04 K and the tolerance the sentence's
own precision licenses is 0.05 K, so neither the prose nor the claim discriminates the
frame — the perturbation that moves the read instant to 155.5 comes back **green**, which
is recorded in the claim's note rather than left to be found. A tolerance tight enough to
separate the first reading from the third would be pinning 0.011 K on a sentence that gives
one decimal: the "right but unreachable" defect this file exists to keep out.

So the frame is chosen on meaning. "When it fires" is an instant; both readings are taken
at it; and that subtraction is the one the page itself performs, contemporaneous by
construction. What is *not* guarded is the two frames swapping, and the note says so.

The trip mechanism did reproduce exactly, which is worth stating separately from the claim:
the probe reads 343.03 K at 155.0 s, 343.2458 K at 155.5 s, and the contactor opens on the
step ending at 156.0 s. One step of lag, visible in the numbers, exactly as the sibling step
says.

## The two refusals, and the gate that is a forward guard

A sensor quantity can be asked for in three places it does not exist, and each is refused by
name rather than answered with something plausible:

1. **On a pack with no BMS.** No sensors at all. The page prints a sentence there instead of
   a number, and this panics instead of returning zero.
2. **On the zero-length probe.** A `dt = 0` step samples nothing, so the frame is the
   construction-time open-circuit read — every probe at the pack's initial temperature, on a
   pack that is uniform anyway. The gap it would report is an exact zero *by construction*,
   which is the false-agreement twin of the false accusation `web/app.js` documents at its
   own `booted` gate. The refusal quotes that gate.
3. **On a stale frame.** Kept and **labelled as a forward guard**, because deleting it
   reddens nothing: sampling is gated on `dt > 0`, so every stepped row carries a frame
   sampled at its own instant, and the one row that does not is refused by (2) first. It is
   the one way this quantity stops being a subtraction of two readings of one instant, and
   the page carries the same comparison for the same reason.

## The literal, merged

The sibling claim's literal used to stop before the word "so", because the fragment after it
named a number nothing could account for. It now runs through the whole clause and both
claims share it, which is what a sentence group is for. Deleting the new claim reddens check
6 by name — `it prints 1.3, and none of the 1 claim(s) on it accounts for that number` — so
the merge is load-bearing rather than cosmetic.

`343.15` is still outside the literal. It is a threshold this step's scenario declares, and
the accounting has no arm for a configured constant — one of the two arms
`docs/plans/path-prose-ledger.md` designs and neither of which exists. That cut is the same
one `the 2.50 V cut-off` and `96 s after the short` already cost.

## Perturbations

Eight cases, each asserting its own anchor matched exactly once before running, at
below-normal priority through `subprocess.run` — never `start /wait`. Two failure texts were
read by hand rather than trusted from an exit code.

| perturbation | result |
| --- | --- |
| point the claim at `t_max_at` instead | value check, by 345 K |
| `probe = true` | the boot-read refusal, in its own words |
| `probe = true` with `read_at_s = 0` (the pre-existing probe fence satisfied) | still the boot-read refusal |
| delete the claim | check 6, naming `1.3` |
| flip the sign of `value` | the `magnitude` fence — "not negative" |
| read it on the `bms off` arm | the no-BMS refusal, naming the instant |
| move the read instant to the deciding frame (155.5 s) | **green** — the recorded limit above |
| delete the staleness gate | **green** — the forward guard above |

Both greens are results, not gaps that went unnoticed: each is stated in the claim's note or
in the code beside the assertion.

## Deferred, with a price

* **The BMS panel's strings are still mirrored by nothing.** `1.31 K` is on the screen — the
  panel's `temperature` row prints the gap — but this file asserts the strings of `READOUTS`
  and that panel is not in it. So the claim is value-only, and the truth-beside-belief
  columns a reader is actually looking at are checked by nothing. This is a narrower gap
  than `soc_gap_pts_at`'s, whose panel prints no gap row at all, and it is the one that
  would close first if the mirror grew.
* **Only the temperature channel is carried.** The current and charge channels of the same
  panel have no claim on them, so building them would be fields with no consulting code.
  The next sentence about the current sensor is where they get built.
* **The frame is not guarded.** Stated above and in the note: the two readings 0.5 s apart
  both print 1.3, so the claim cannot tell them apart, and nothing would redden if the
  engine started deciding from the contemporaneous frame instead.
* **`343.15` is still unaccountable**, along with every other configured constant a claimed
  sentence prints. Unchanged by this slice, and now the only reason this sentence's literal
  is a fragment rather than the whole thing.
