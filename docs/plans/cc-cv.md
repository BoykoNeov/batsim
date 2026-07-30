# CC-CV: the other half of the demand story, and the leg LFP does not have

Item 1 of the queue `docs/plans/scenario-catalog.md` left open. Not a numbered phase —
Phases 0–6 built an engine, and this is the fourth slice of the client catching up to
it, on the same footing as the `ui-*.md` slices and the catalogue.

Everything a reader can do to a pack today takes charge **out** of it. Five scenarios,
eight lesson steps, and not one of them charges anything. `Demand::Voltage` has existed
since Phase 0, is exercised by four test files, and is reachable from the page only as a
number a reader would have to hold by hand at exactly the right moment. `CLAUDE.md` says
where the missing piece goes: *"`Demand::Voltage(V)` … used for CV charging; combined
CC-CV is a client-side policy."* This slice writes that policy.

## What is missing, stated as source

`web/app.js:1404` and `:1442` — the whole of the demand path:

```js
const mode = $("demand-mode").value;
const demand = demandJson(mode, Number($("demand-value").value) || 0);
```

One demand, read once, applied to every step of the frame. A CC-CV charge is two demands
with a condition between them, so this is the line the slice has to change — and the
*way* it changes is the entire design problem below.

Second, `scenarios/`: every one of the five files starts at 85–100 % SOC. There is no
pack in the repo that a charge could be demonstrated on.

## The measurement that came before the design

A throwaway harness against `sim-core` (in the temp tree, not committed), reading
coefficients from `chemistries/*.toml`. Recorded first, because this repo has twice paid
for a claim written from reasoning, and because two of the four things below killed a
design that was already drafted.

### 1. LFP has no CV leg, and that is the second half of a lesson already shipped

CC-CV to each cell's own `v_max`, 1S1P, from 20 % SOC, 25 °C, taper at 0.05 C:

| chemistry | CC leg | SOC at the knee | CV leg | ends at |
|---|---|---|---|---|
| `nmc_18650_generic` @0.5 C | 5420 s | 0.9528 | 790 s | 0.9952 |
| `nmc_18650_generic` @1.0 C | 2510 s | 0.8972 | 1080 s | 0.9953 |
| `nmc_21700_lgm50` @1.0 C | 1920 s | 0.7333 | 2410 s | 0.9938 |
| `lfp_26650_generic` @0.5 C | **never reaches 3.65 V** | — | — | SOC clamp at 5760 s |
| `lfp_26650_generic` @1.0 C | 2880 s, at SOC 0.9999 | 1.0000 | degenerate | SOC clamp |

The LFP cell tops out at **3.6357 V — 14.3 mV short of its own 3.65 V limit** — and then
sits there while the coulomb counter clamps. The arithmetic is the plateau: this file's
`[ocv]` ends at 3.60 V, its `[r0]` is ~21 mΩ at the top, and 0.5 C is 1.15 A, so
3.60 + 0.024 never crosses 3.65. **The pack fills before it reaches its voltage limit.**

State it as this parameter set's behaviour, not as a fact about LFP cells: real LFP is
charged CC-CV to 3.65 V, and both halves of that arithmetic — where the OCV table ends
and how big R0 is — are hand-fitted numbers in a file that says so. What is *not*
parameter-dependent is the shape of the argument: a chemistry whose OCV barely moves has
almost nothing left for a CV leg to do. Steps 1 and 2 of the guided path already teach
the discharge half (168 mV of span against NMC's 481); this is the charge half, and it
falls out of the same table.

### 2. The memoryless rule chatters, and the chatter changes the answer

The first draft of the controller was `v ≥ target → CV, else CC`, with no state — chosen
precisely because a phase flag would live outside `Snapshot` and a restore would
resurrect the wrong leg. It is wrong as written, and the run says so:

`nmc_21700_lgm50` at 1.0 C, plain `>=`: **1311 CC/CV flips**, a terminal voltage peaking
at **4.327 V — 127 mV above a 4.20 V target** — and a charge that terminated at
**t = 3158 s, SOC 0.9653** instead of **t = 4322 s, SOC 0.9937**. Not a cosmetic wobble:
1160 s and 2.8 points of charge, because one chattering step happened to fall below the
taper cutoff and the controller called it done.

The cause is an exact-equality boundary. **The CV solve does not land on the target; it
lands about 1.4 × 10⁻⁴ V away from it**, and the residual shrinks as the current tapers
(−1.44e-4, −1.41e-4, −1.38e-4 … in consecutive steps). The moment it crosses to the other
side of the target the rule flips to CC, one CC step re-establishes the full `I·R0 + ΣV_rc`
overpotential the CV leg had let decay — which on that cell at 5.15 A is 127 mV — and the
next step flips back.

Fix: compare against a **band**, not the target. `v ≥ v_target − ε` with `ε = 1 mV per
series cell`. Measured across NMC and LG M50 at 0.5 C and 1.0 C:

| ε | flips | peak above target | when the knee lands |
|---|---|---|---|
| 0 to 1e-6 V | 1 or **1311** | 0.07 mV or **127 mV** | correct, or chaos |
| 1e-4 V | 1 | 0.15 mV | 3.5 s early |
| **1e-3 V** | **1** | **0.17 mV** | **3.5 s early** |
| 5e-3 V | 1 | 0.33 mV | 26 s early |

1 mV/cell is two orders above the residual and three below anything a reader can see.
The controller stays memoryless; that property was worth keeping and it survives.

### 3. The sub-clock costs almost nothing, so the controller stays in the client

The decision point must not depend on how a frame happened to be chopped: a frame of 500
steps and two frames of 200 + 300 would otherwise evaluate the controller at different
step indices, and the same scenario at the same `dt` would take a different trajectory
depending on wall-clock timing. That is `CLAUDE.md` principles 3 and 4 in one defect.

So decisions happen **only at multiples of a fixed simulation-time period**, exactly as
aging's `sub_clock_period_s` already does. What that quantisation costs at the knee:

| decision period | NMC 0.5 C | NMC 1.0 C | LG M50 1.0 C |
|---|---|---|---|
| 0.5 s (every step) | +0.11 mV | +0.17 mV | +0.17 mV |
| 5 s | +0.11 mV | +0.13 mV | +0.16 mV |
| **10 s** | **+0.11 mV** | **+0.13 mV** | **+0.84 mV** |
| 30 s | +0.53 mV | +2.40 mV | +0.84 mV |
| 60 s | +4.71 mV | +2.40 mV | +0.84 mV |
| 120 s | +13.07 mV | +2.40 mV | +0.84 mV |

**10 s**, matching the aging sub-clock's own default. Sub-millivolt everywhere, and the
end state is unmoved (SOC 0.99520 against 0.99519 for a per-step controller).

The excursion is **not monotone in the period** — 30/60/120 s give an identical
+2.40 mV on NMC at 1 C — because the peak depends on where the decision grid happens to
land relative to the crossing, not on the window length alone. Do not write it as a
monotone trade-off.

This is the number that decides the architecture. A knee that is accurate to a
millivolt at a 10 s decision period means **the controller belongs in the client and no
Rust moves** — no new `Demand` variant, no controller in two adapters, no version bump.
Had it cost tens of millivolts at speed, the honest answer would have been to put it in
`sim-wasm` and `sim-server` and pay `API_VERSION` for it.

And the property that matters is directly checkable: the same run, split into backend
calls of 1, 3, 7, 20, 333 and 100 000 steps, ends **bit-identical** (SOC
`0.99519515334915398`, peak `4.20010593256333031`, done at t = 6210.000000 in all six).

### 4. Charging reaches four flags that no discharge in this repo has ever raised

Measured on the candidate pack scenario of Part B (4S2P NMC, scatter, thermal network,
BMS with balancing, from 40 % SOC):

| run | what happens |
|---|---|
| 0.5 C, 25 °C, BMS **on** | `BALANCING` from t = 3111 s; **`OV` at 3986 s** and the current stops — at SOC **0.9514**, with the top cell at 4.20004 V and the bottom at 4.18901 V |
| 0.5 C, 25 °C, BMS **off** | runs to the taper: SOC **0.9952** at 4820 s |
| 1.0 C, 25 °C, BMS on | `OC` **from the first step**, current derated to exactly 4.200 A = 0.7 C × 2P; `OV` at 2729 s, SOC 0.9321 |
| 0.5 C, −5 °C, BMS on | pack cools; `UT` at 1494 s and charge stops at SOC **0.6075** |
| 0.5 C, −5 °C, BMS off | `PLATING_RISK` at 1494 s — the same instant — and it charges on to 0.9921 |

`BALANCING`, `OC`, `OV` and `PLATING_RISK` have all been reachable since Phases 2 and 3
and none of them has ever been reachable *from the page*: a discharging pack raises none
of them. The BMS-on/BMS-off pair at 0.5 C is the cleanest protection lesson in the repo —
**4.4 points of charge is what the protection costs, and it costs them because one group
of four reached 4.20 V while the pack as a whole had not.**

The instant to quote is the step *before* the trip, and getting that wrong was worth
105 mV: at t = 3985.5 s the pack is at **16.775 V, 24.8 mV short** of its 16.80 V target,
with its cells spread over **11.0 mV** — 4.18901 V at the bottom, **4.20004 V at the
top**, over the limit. One step later the contactor is open and the terminal reads
16.670 V, 130 mV below target, because the IR drop went away with the current. The first
draft of this plan quoted that 130 mV as the imbalance. It is not; it is the load coming
off, and the imbalance is the 11 mV.

Two things this measurement *killed*:

- **The one-quadrant charger guard is out.** Drafted as a third leg — "a charger cannot
  sink current, so `Rest` when the pack is above the target" — because a CC-CV mode that
  *discharges* a pack whose voltage already exceeds the target reads as a bug. Measured,
  it produces a CC/Rest limit cycle: after a knee overshoot the rested terminal voltage
  is the OCV, which is below the target, so the next window charges at full current, which
  overshoots again, and the pack never enters CV at all. The distinction the rule needs —
  *is the pack above the target, or merely at it* — is unobservable once CV is holding
  the terminal at the target. Rejected in favour of a two-way rule plus a warning in the
  status line when the target is set below the pack's present voltage.
- **The pack scenario is NMC, not LFP.** Drafted as a 4S2P LFP because the only
  BMS-carrying scenario in the repo is LFP; but §1 says an LFP pack never reaches its
  CV leg, so the protection lesson would have been tangled up with a knee that never
  arrives. On NMC all five rows above are clean.

## Part A — the controller

Three rules, in `web/app.js`, evaluated **only** at decision points.

**When.** `period_s = 10`, converted to a whole number of steps `K = max(1, round(10/dt))`,
and anchored to **simulation time, not to a client-side counter**: the decision index is
`round(sim_time_s / dt)`, and a decision is due when that index is a multiple of `K`.
Anchoring to `sim_time_s` — which the backend reports and a snapshot carries — means a
restore lands back on the same decision grid without the client remembering anything.
There is no controller state to reset on load, restart or restore, which is the
generation-counter list from the last slice and the one this design is built to stay off.

**What.** With `v_target = v_cell × series` (read from `facts()`, so it follows the loaded
topology) and `ε = 1 mV × series`:

```
v < v_target − ε   →  Demand::Current(−i_charge)     // CC; negative = charge
otherwise          →  Demand::Voltage(v_target)      // CV
```

`v` is the last telemetry frame's `v_terminal` — one step stale, which is inside the
band by three orders of magnitude.

**Done.** Checked at the same points, from `|i_actual| ≤ i_taper`, and it reports *why*,
because §4 shows the same condition arriving three ways:

- any of `UT OT OV UV OC CONTACTOR_OPEN` set → **"stopped by the BMS"** plus the flags.
  A pack that stops at 60.8 % SOC because it got cold must never be labelled "charged".
- in CV, no protection flags → **"complete — tapered to X A at Y %"**.
- in CC → the charge current is at or below the taper cutoff, which is a misconfiguration,
  and saying so beats a charge that reports success at t = 10 s.

`i_actual` is the post-BMS current, which is exactly why the flag check cannot be skipped.

**How the frame is split.** `advance()` currently makes one backend call per frame. It
becomes: while steps remain, take `min(steps_left, steps_to_next_decision)`, decide once,
call the backend, repeat. Decimation is computed per window from the frame's total so a
frame's report budget is unchanged; each window contributes at least one frame, and the
existing `maxFrames` guard stays. At 10⁴× with `dt = 0.5` a frame is ~333 steps ≈ 17
windows, so the socket path pays ~17 round trips per frame at top speed — measured on the
socket during verification rather than assumed, and reported either way.

`readNow()`'s `dt = 0` probe has no previous frame to decide from and uses the CC leg,
which is what a pack at any state below the target would get anyway; the first real
window corrects it. Stated in a comment rather than left to be discovered.

## Part B — the scenarios

Three files, each carrying the header comment the existing five set the standard for, and
each an **initial condition only** — the controller above is the client's, per
`scenarios/cc_discharge_lfp.toml`'s own header. The names say what they are *for*, which
is the convention `cc_discharge_*` already set.

1. **`cc_cv_charge_nmc.toml`** — 1S1P `nmc_18650_generic` at 20 % SOC, 298.15 K,
   isothermal, no BMS, no aging, no faults. Differs from `cc_discharge_nmc.toml` in
   exactly one field, `initial_soc`, so the charge and the discharge are the same cell.
2. **`cc_cv_charge_lfp.toml`** — the same again for `lfp_26650_generic`, and its header
   carries §1: at 0.5 C this cell never reaches 3.65 V, and that is the point of the file
   rather than a defect in it.
3. **`cc_cv_charge_pack.toml`** — 4S2P `nmc_18650_generic` at 40 % SOC, 298.15 K, seeded
   scatter (`capacity_sigma = 0.02`, `r0_sigma = 0.03`), `[pack.thermal.Network]`, and a
   BMS with protection **and** balancing at `v_threshold_v = 4.10`. The first NMC pack in
   the repo and the first BMS the page can watch *protect* something. Thermal coupling is
   load-bearing: the cold lesson works by dragging the ambient slider and letting the pack
   cool through 0 °C, which an isothermal pack cannot do.
   - At rest the eight cells read **3.75000 V, spread 0.00 mV** — scatter is in capacity
     and R0 factors, and SOC is uniform, so it shows nothing until current flows. The last
     slice learned this on a resting aging pack; here the charge makes it visible (13.3 mV
     of spread at 0.5 C), which is the same fact from the other side.

A test walks `scenarios/*.toml` and parses each; it exists already (`crates/sim-data/tests/scenario.rs`)
and these three inherit it.

## Part C — the client

**A fifth demand mode, `CcCv`.** Its own field group — charge current [A, positive],
target [V per cell], taper [A] — shown in place of the single `demand-value` input, plus
a status line that names the leg, the present current, and the completion reason from
Part A. The sign trap is handled by the field's own label: everywhere else on this page
positive is discharge, and the charge current is entered positive *because the mode is a
charger*. The controller negates it; the plot still shows a negative current, which is
the convention and is worth one sentence in the first lesson.

**Three lesson steps**, appended to `LESSONS` — records, no new markup, path 8 → **11**.
The `demand` record grows two optional fields (`v_cell`, `taper`) that only `CcCv` reads.

- *The two legs* — `cc_cv_charge_nmc.toml` at 0.5 C. CC for 5420 s to 95.3 %, then the
  knee, then 790 s of CV that adds the last 4.2 points at a current falling from 1.5 A to
  0.15 A. **13 % of the time for 5 % of the charge** is the number that makes the shape
  worth understanding.
- *The leg that is not there* — `cc_cv_charge_lfp.toml`, same controller, same rate. The
  terminal voltage stalls at 3.6357 V, the mode stays in CC forever, and `SOC_CLAMPED_HIGH`
  appears at 5769 s. The clamp is also where this engine's known overcharge hole lives —
  the coulomb counter stops but the current does not — and the step says so rather than
  letting a reader infer that a full pack accepting 1.15 A is physics.
- *What the BMS costs, and why* — `cc_cv_charge_pack.toml` at 0.5 C with the BMS on
  (`OV` at 3986 s, 95.1 %), then the same run with it off (0.9952 at 4820 s), then the
  ambient slider to −5 °C (`UT` at 1494 s, charge stops at 60.8 %) and the same cold run
  with the BMS off (`PLATING_RISK` at the same instant, charges on regardless).

**Two counters must move with the path**: the `path-start` button label in
`web/index.html` and the "walks eight steps" sentence in `README.md:188`. The last slice
shipped a README still counting six.

**No version constant moves.** `API_VERSION` — no route, field or error code changes;
`WASM_API_VERSION` — no engine call changes shape; `SNAPSHOT_VERSION` — no engine state
changes. Each read from its own doc comment rather than assumed, which is the correction
this repo paid for in `ui-bms-view.md`.

## Verification, and the traps this page has already sprung

- **Every new control gets a real press.** The mode option, all three fields, and the
  three new lesson steps by clicking Next — the previous two slices both shipped a wrong
  note because a transition was only ever driven from code.
- **Both transports, and the socket really driven**, not inferred from the wasm run: the
  window-splitting change lands on the call path the socket serialises, so ~17 calls per
  frame at 10⁴× is a claim about the socket that only the socket can settle.
- **Batch-independence is checked on the page, not only in the harness**: the same
  scenario at two speed multipliers must agree on `sim_time_s` at the knee and on the
  final SOC. That is the property the sub-clock exists for and no unit test covers it.
- rAF does not fire under an occluded automation window; one screenshot buys one frame;
  never `await setTimeout` in an injected script; anything read at a high multiplier is
  re-read at rest before it becomes prose.
- A 1 h 43 m charge cannot be watched under occlusion — every number quoted in prose comes
  from a native run of the committed scenario files, and the page check is for controls,
  legs, and the switch.
- `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo fmt --all` before the commit, per `CLAUDE.md`. No Rust changes are expected; the
  gate runs anyway.

## What the build changed

Written after the fact, as the last four plans were.

**The status line's first act was to lie about a derate.** Switching the mode to CC-CV
with the page holding a frame from a 2 A *discharge*, it read: *"constant current: 2.000 A
in, terminal 3.596 V of 4.200 V. Asked for 1.500 A — something is limiting it."* Two
faults in one sentence — "in" for a current flowing out, and a derate diagnosed from a
frame taken under a different demand. Fixed at both ends: the shortfall test is now
`i_actual < 0` (a derate is only a derate if the pack is actually charging), and
`demand-mode`'s `onchange` takes a `dt = 0` probe so the readouts describe the demand
just selected. The guided path already did the second thing after setting a step's
controls; a reader turning the knob by hand deserved it too.

**The frame-independence check came out better than it was planned.** The plan asked for
two speed multipliers agreeing. What was actually run is stronger: `cc_cv_charge_nmc`
over the **WebSocket** at 10 000×, in frames of thousands of steps arriving at whatever
irregular cadence a screenshot-driven automation window produces, finished at
`sim_time_s` = **6210.0** — the same instant, to the tenth of a second, as the native
harness stepping one step at a time. The page's own splitting code lands on the same
decision grid as the algorithm it implements. The wasm path agreed independently
(104 m displayed, 99.5 %, −0.150 A).

**The socket pays for the sub-clock, and the number is worth recording.** At 10 000× and
`dt = 0.5` a frame is ~5000 steps, which is **250 decision windows and therefore 250
round trips**. Measured, the socket path advanced ~800 s of simulation per frame where
the in-page engine did ~2500 s. Correctness is untouched — the trajectory is identical,
which is the whole point — but the socket at top speed in this mode is window-bound. The
in-page engine is the default and is unaffected.

**Verified on screen, by hand:** the mode option and all three fields; the two legs
(`−1.500 A` flat, then the knee, then `0.604 A and falling`, then *complete — tapered to
0.150 A, at 99.5 %*, with the run stopping itself); the LFP step stalling at 3.636 V with
`SOC_CLAMPED_HIGH` and a status line still reading `constant current` at 100 % SOC; the
pack step's `OV` + `BALANCING` at 67 m with *stopped by the BMS — OV. The pack is at
95.1 %*, terminal 16.670 V and balancing 2.106 W — both to the digit the harness
predicted; the same pack with the BMS off reaching *complete — tapered to 0.298 A, at
99.5 %*; and the derate, by typing `6` into the charge-current field and taking one step:
`−4.200 A`, `OC`, and *Asked for 6.000 A — something is limiting it.*

**Not driven on screen, stated rather than glossed:** the cold-charge half of step 11
(ambient to −5 °C → `UT` at 1494.5 s and 60.75 %, or `PLATING_RISK` at the same instant
with the BMS off). Those numbers come from a native run of the committed scenario.

**The select-dropdown trap recurred**, and the last slice's note was right to say *do not
retry*: after the first page reload, arrow keys stopped reaching a focused `<select>`
entirely and the native dropdown does not render into a screenshot of an occluded window.
Everything after that point was driven with buttons, checkboxes, sliders and typed text
fields, which all work. The CC-CV `<option>` itself was selected by hand once, before the
reload; every later selection came from the guided path's applier pressing the same code
path.

## Exit criterion

A reader can charge a pack from the page, see the current fall away at the knee without
touching a control, watch a BMS stop a charge 4.4 points short and be told why — and the
same charge at 1× and at 10⁴× produces the same trajectory.
