# Protection escalation: the clamp that cannot reach a short, and the contactor that can

The sixth slice of the client catching up to the engine, and the first to make the
**contactor** visible. Not a numbered phase: Phase 2 built graduated protection and
Phase 3 built the fault queue; this is the first time a reader can watch the escalation
from *derate* to *open*, which is the one step of that ladder no scenario and no lesson
in the repo has ever reached.

## What is missing, stated as source

`crates/sim-core/src/bms.rs` documents a two-rung response — derate, then latch the
contactor open — and the second rung is unreachable from the browser. The guided path's
fourteen steps reach `OC`, `OV`, `UV`, `UT`, `BALANCING`, `PLATING_RISK`,
`SOC_CLAMPED_HIGH` and `SOC_CLAMPED_LOW`; **`CONTACTOR_OPEN` and `OT` appear in none of
them.** The engine's own tests do cover both (`tests/scenario_protection.rs` asserts
`CONTACTOR_OPEN | OT` at line 378), so this is a client gap, not an engine one.

`crates/sim-core/src/faults.rs` names the missing experiment in its module docs, and has
named it since Phase 3:

> [`Fault::ExternalShort`] is a resistance across the pack's **load-side** terminals,
> downstream of the main contactor. Opening the contactor therefore interrupts it. That
> choice is deliberate: it is what makes the BMS-on / BMS-off contrast a real experiment
> (protection sees the voltage sag, derates, and eventually latches the contactor open,
> saving the pack — where an unprotected pack runs the short until it empties or
> overheats).

`ExternalShort` appears in nine engine tests and in **no scenario file**. Ten files in
`scenarios/`, and the only fault any of them injects is the soft internal short.

Nothing else was missing. The reset-seam UI (`#fault-clear-bms`, wired to
`Pack::clear_bms_fault` over both transports) and live injection of an `ExternalShort`
with an arbitrary resistance were both built in the UI pedagogy slice and have callers
already. This slice adds **two scenario files and two lesson steps**, and fixes one line
of wrong prose the measurement caught.

## The measurement that came before the design

A throwaway harness under `M:\claud_projects\temp\protection-sweep`, not committed,
against `sim-core` and `sim-data` directly. Every number below is at the page's own
`dt = 0.5 s`, because every one of them is going into prose a reader will check against
their screen.

**One convention to hold on to, because it is half a step wide and it bit this slice.**
The harness labels each line with the time at the *start* of its step; a `Frame` — on
both transports — carries `sim_time_s` read *after* the step, "so a frame describes the
pack at the end of the step that produced it". Every time in this document is therefore
0.5 s earlier than the same instant displayed on the page, and the lesson prose quotes
the page's number, not this one. The latch that appears below at t = 155.5 reads
**156.0** on screen. The pack is `soft_short_under_a_lying_sensor.toml`'s — 4S2P LFP, scatter,
thermal network, two temperature probes on corner cells, protection margins 0.15 V and
10 K — at 90 % SOC, resting, with one `ExternalShort` at t = 60 s and no other fault.

### The sweep: three regimes, and the design lives in two of them

Twelve resistances from 10 mΩ to 3 Ω, at `dt = 0.1 s`, asking only "does it latch, when,
and having lost what":

| R \[Ω] | latch | SOC at latch | peak current | peak cell temp |
|--------|-------|--------------|--------------|----------------|
| 0.01–0.05 | one step after the short | 0.899 | 254 → 144 A | 298.5 K |
| 0.08 | +70.4 s | 0.467 | 108.6 A | 344.2 K |
| 0.12 | +124.8 s | 0.323 | 81.8 A | 344.8 K |
| 0.20–0.80 | +284 s … +987 s | 0.03–0.01 | 54.8 → 15.7 A | 342 → 308 K |
| 1.50, 3.00 | never (4 h) | — | 8.6, 4.4 A | 302, 299 K |

Three regimes, and the two that teach something are the extremes of the top half. Below
about 50 mΩ the sag alone crosses the hard voltage margin and protection wins outright.
Around 100 mΩ **it does not**, and what eventually stops the pack is not the voltage path
at all. Above 200 mΩ the latch arrives only when the pack is already empty, which is a
third story and not one this slice tells.

### A: 30 mΩ — the sag latches, and one step gets through

```
t=  59.50 sensed(v_lo= 3.3142) -> i=    0.00A v_term= 13.257 soc=0.9000 t_max= 298.15K
t=  60.00 sensed(v_lo= 3.3142) -> i=  183.84A v_term=  5.498 soc=0.8944 t_max= 299.11K  (no flags)
t=  60.50 sensed(v_lo= 1.3336) -> i=    0.00A v_term= 13.168 soc=0.8944 t_max= 299.11K  UV | CONTACTOR_OPEN
```

The short fires at the top of the step at t = 60.0 and **183.84 A** flows for that whole
step with no flag raised, because the BMS decides from the frame sampled at the end of
the *previous* step, which still reads a resting 3.3142 V. The frame taken at the end of
this step reads **1.3336 V**, below `v_min − v_hard_margin` = 2.00 − 0.15, and the next
step latches. Cost of the whole event: **0.56 SOC points, 0.96 K, and one 0.5 s spike.**
SOC is then frozen at 0.8944 for good — the contactor is upstream of the short.

This is `bms.rs`'s documented one-step sampling lag ("the first step of an excursion is
not prevented, only the ones after it") made visible as a single tooth on the current
plot. It is the first step in the path whose headline quantity is *one step long*, which
is why the lesson pins `dt`.

### B: 100 mΩ — the sag does not latch, and neither does anything else for 73 s

```
t=  60.00 sensed(v_lo= 3.3142 t_hi= 298.15) -> i= 93.29A v_term= 9.292 soc=0.8972  (no flags)
t=  90.00 sensed(v_lo= 2.1598 t_hi= 313.19) -> i= 87.02A v_term= 8.700 soc=0.7354  (no flags)
t= 133.00 sensed(v_lo= 2.1326 t_hi= 333.31) -> i= 85.90A v_term= 8.590 soc=0.5101  OT
t= 155.50 sensed(v_lo= 2.0993 t_hi= 343.24) -> i=  0.00A v_term=11.328 soc=0.3962  OT | CONTACTOR_OPEN
```

Three separate reasons nothing trips, and they compose:

1. **The sag is not deep enough.** Sensed group voltage bottoms at ~2.13 V. That is above
   the hard margin (1.85 V) — and above `v_min` itself (2.00 V), so not even the *soft*
   under-voltage rung fires.
2. **The current is not a demand.** `Bms::apply_protection` judges over-current against
   `i_req`, the current the demand solved for. The demand here is `Rest`. Nobody asked
   for 86 A, so no `OC` is raised — and, more to the point, **the derate has nothing to
   act on**: clamping the demand window to zero does not remove a conductance across the
   terminals. The first rung of the ladder cannot reach this fault at all.
3. **So the pack cooks instead.** 86 A through the pack's own resistance for 95 s takes
   the hottest cell from 298 to 344.5 K. The first flag of any kind is `OT` at
   **t = 133.0**, 73 s in and half the charge gone.

The latch comes at **t = 155.5 s** — 95.5 s after the short — when a *probe* crosses
`t_max_k + t_hard_margin` = 343.15 K. The probes sit on cells (0,0) and (3,0); ground
truth's hottest cell is at 344.52 K when the probe reads 343.24. The BMS trips late by
1.3 K of somebody else's temperature, which is the truth-versus-belief theme arriving in
the protection path.

SOC at the latch is **0.3962**: the pack lost **50.4 points** to a fault the voltage rung
never saw. Against regime A's 0.56, that is a factor of 90 for a resistance three times
larger.

### C: the control — 100 mΩ with the BMS off

Bit-for-bit the same trajectory as B until B latches, which is itself the point: the BMS
changes nothing until the moment it acts. Then it runs on. `SOC_CLAMPED_LOW` at
t = 235 s, and after that **the current does not stop** — ~50 A forever with SOC pinned at
zero and the hottest cell past 404 K at t = 600 s.

That tail is the **discharge side of the overcharge energy hole** that
`docs/plans/phase-3-aging-faults.md` left open deliberately: at SOC 0 a cell keeps
sourcing current at `OCV(0)` because nothing models an empty electrode. The step's prose
must say so and must not build a runaway claim on it — the pack never reaches
`t_onset_k` = 423.15 K inside the lesson's window, and the heat it *does* produce past
t = 235 s is the model's hole, not the pack's energy. Step 10 already points at the
charging half of the same hole, so this is the second face of a defect the path already
admits to.

> **Amended by `energy-hole.md`.** The two faces have since parted company. The
> *charging* half is fixed — refused charge is dissipated at `OCV(1.0)` and step 10's
> prose was rewritten — while the **discharge half above is still exactly as described**,
> and is now *reported* rather than merely admitted to: `Telemetry::i_rejected_a` is
> positive throughout that tail and a property test pins the fabricated energy to it.
> Nothing in this section's measurements moved; the low clamp adds no heat, by decision.

### D: the reset seam, and why it is a two-step

```
clear_bms_fault() -> true          (short still connected)
   +0 i=  184.63A   flags=(none)
   +1 i=    0.00A   flags=UV | CONTACTOR_OPEN
clear_faults() -> 1 removed
clear_bms_fault() -> true
   +0 i=    0.00A v_term=13.162   flags=(none)
```

Closing the contactor onto a live short costs **another full step at 184.63 A** and
re-latches immediately — the same sampling lag as the first time, and the reader can see
the second tooth on the plot. Remove the fault first and the contactor stays closed. The
order is the lesson; a single button pressed in the wrong order is a repeat of the
accident.

### The measurement also caught a wrong line of UI prose

`#fault-clear`'s note reads:

> dropped N queued fault(s). **Faults that already fired stay in effect** — clearing the
> queue is not a repair.

That is false for everything except a fired `WeakCell`. `Pack::clear_faults` clears the
queue *and* the external short *and* every cell's internal-short conductance *and* the
sensor corruptions — its own doc comment calls it "Repair the pack" and names `WeakCell`
as the single exception. Run D is the proof: the queue was empty by t = 100 s, the call
returned `1`, and that 1 was the *fired* short — after which the current went to zero and
stayed there. The note is corrected to say what actually persists, since this slice sends
a reader to press that exact button.

## Design

**Two scenario files**, twins differing in one number — the Phase 3 rule that an
emergent-failure claim needs a control differing in one coefficient, applied to a fault:

* `scenarios/external_short_30_milliohm.toml`
* `scenarios/external_short_100_milliohm.toml`

Both are the soft-short scenario's pack at `initial_soc = 0.90` with the same seed, both
schedule one `ExternalShort` at t = 60 s, and the only difference between the files is
`ohms`. `GET /scenarios` reads the directory, so both reach the picker with no server
change, and `scenarioSummary` already labels them by topology and BMS.

**Two lesson steps**, taking the guided path from 14 to 16:

1. *A short across the terminals, and the one step that got through* — 30 mΩ, `Rest`,
   BMS on. The spike, the latch, the frozen SOC, and the invitation to clear the latch
   with the short still live (another spike) and then in the right order (nothing).

   Three details of that invitation are page behaviour, not engine behaviour, and each
   was wrong in the first draft. The buttons are labelled **Clear latched BMS fault**
   and **Clear queued**, so the prose must use those words rather than describing what
   they do. The page is *paused* at a step's mark and clearing a latch does not step, so
   the second spike needs an explicit Run. And re-running with a different `dt` must go
   through **Restart** — `restart(bms)` rebuilds from the scenario at t = 0, re-queueing
   the fault, and leaves the controls alone — because Back-then-Next re-applies the
   step's whole control set and the new `dt` field would undo the very change the reader
   was invited to make. A step that hands out instructions is a control path of its own
   and has to be walked, not reasoned about.
2. *The same short, three times weaker, and nothing that will clamp it* — 100 mΩ, `Rest`,
   BMS on, wasm transport so the BMS-off contrast can rebuild the pack in place. 73 s of
   silence, the thermal path arriving instead of the voltage one, the probe trailing
   truth, and the honest note about the tail past SOC 0.

**One client change beyond the lessons:** lesson records gain an optional `dt`, applied
through `#dt` like every other control, and both new steps pin `0.5`. Every existing
step's numbers already assume that value implicitly; these two are the first whose
headline quantity *is* a step, so leaving it to whatever the reader last typed would let
the prose be wrong by a factor.

No Rust changes, so no `web/pkg` rebuild. `sim_server::API_VERSION` and
`WASM_API_VERSION` both scope to wire and method shape and neither moves;
`SNAPSHOT_VERSION` stays at 10.

**Deliberately not touched:** the five hard-coded `<option>`s in `index.html` are a
degraded-mode fallback for a page served without the server, and they have not tracked
the scenario directory since the CC-CV slice — they now name five of twelve files. Every
slice since has left them alone and this one does too, because the moment the server
answers `GET /scenarios` they are replaced wholesale. Worth its own decision some day
(either fill them or cut them to one), not worth making it here.

## Exit criterion

The guided path runs 16 steps. A reader can watch a dead short latch the contactor one
step after it fires, having cost the pack half a percent — then watch a weaker short of
the same kind cost it fifty percent and 46 K because the derate has nothing to clamp, and
see the trip arrive from the thermal path on a probe that reads 1.3 K cooler than the
cell that is actually hot.
