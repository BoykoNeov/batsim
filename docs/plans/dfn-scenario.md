# The DFN scenario: a rate at which the particle model stops knowing the cell is dying

The last item on the list `docs/plans/dfn-aging-gap.md` closes with, and the direct
counterpart of `docs/plans/spm-scenario.md` one phase later:

> No DFN scenario file, so no client can reach the model — the Phase 6 SPM slice's
> counterpart has never been written for Phase 7.

Not a numbered phase. Phase 7 built the Doyle–Fuller–Newman cell over four slices and
closed both its exit criteria; this is the client catching up to it, exactly as the SPM
scenario slice was the client catching up to Phase 6.

## What is missing, stated as source

`crates/sim-core/src/dfn.rs` is a full DFN — four equations, an analytic banded Jacobian,
a damped Newton solve, goldens against PyBaMM at 1C and 3C — and it is **reachable from
no client**. `PackConfig::cell_model` is settable only from a scenario file, and of the
twelve files in `scenarios/` exactly one sets it, to `Spm`. `CellModelConfig::Dfn` has
never appeared in a TOML file in this repo.

The same sentence was true of `Spm` until `pulse_train_spm.toml`, and that file's opening
comment is written as a claim about the *engine* ("until this file no client could reach
it") which this slice must not leave standing unqualified — see the prose sweep below.

## What is already measured, and where it is committed

This slice starts from a much better place than the SPM one did. The SPM slice had to
find its discriminating experiment by measurement and killed three candidates doing it.
Here the discriminating experiment is **already committed as a test**, because Phase 7's
exit criterion 2 was exactly "reach a regime an SPM cannot follow".

From `crates/sim-data/tests/dfn_golden.rs`, all at 25 °C, 1S1P, isothermal, no BMS, from
100 % SOC, on `nmc_21700_lgm50`:

| quantity | value | where |
| -------- | ----- | ----- |
| 3C for this cell | **15.459594 A** (`3.0 × 5.153198`) | `CAPACITY_AH` |
| batsim's DFN cut-off at 3C | **464 s, 1.993 A·h** | `dfn_cc_3c_lands_on_the_references_cut_off` |
| PyBaMM DFN reference | 488.7 s, 2.098 A·h | same |
| worst replay error, DFN | 62.1 mV over the first 90 % | `the_spm_cannot_follow_the_reference_into_depletion` |
| worst replay error, SPM | **521.0 mV** — 8.4× | same |
| the SPM at the end of that replay | **3.42 V where the reference is 2.59 V** | same |

That last row is the lesson. The comment beside it already says why, and it is the
sentence the whole slice exists to put in front of a reader:

> it does not know the cell is dying, because the quantity that is killing it is the one
> an SPM holds constant.

And the shape of the effect, from `docs/plans/phase-7-dfn.md`'s spike table (PyBaMM's own
Chen2020 basis, so 3C is 15.0 A there and the numbers are not this repo's): mean |DFN −
SPM| is **11.0 mV at 0.2C, 58.1 mV at 1C, 366.4 mV at 3C**, and the cut-offs agree within
0.3 % at 1C and C/5. **A cliff between 1C and 3C, not a slope.** Of the 899 mV worst-case
gap at 3C, **668 mV is φ_e alone** — the electrolyte potential — against 42 mV in the
positive solid phase and 36 µV in the negative.

So the pedagogy is not "the DFN is more accurate". It is: *there is a regime boundary,
here is which side of it you are on, and here is what you are blind to on the wrong side.*
A reader who drops the current to 5.153 A watches the difference very nearly vanish, and
that is the point rather than a caveat.

## What is not measured, and must be before any prose is written

Three of the last four client slices wrote a claim about "what you will see" from
reasoning and had to correct it after looking. The rule this repo has paid for three times
is **measure first**. Every number below is unknown today:

1. **batsim's own SPM at 3C, free-running.** The committed measurement is a *replay* — the
   SPM driven along the reference's clock — which says how wrong it is, not when it would
   stop. `run_to_cutoff` has never been pointed at an SPM. PyBaMM's SPM answers 1084.7 s /
   4.520 A·h on its own 5.0 A·h basis; batsim's is validated at C/5, 1C and GITT only, and
   **3C is outside that set**. Measure it, and quote it as this engine's answer.
2. **Whether the SPM arm falls off its OCP table before 2.5 V.** `pulse_train_spm.toml`
   warns that past the SOC clamp the model pins at 0.39–0.50 V. At ~4.5 A·h delivered it
   should be near 13 % SOC and nowhere near the clamp — *should*, which is not a
   measurement. If it lands in that hole the lesson's mark must sit before it.
3. **What the DFN does after 464 s.** Nothing stops it: `bms: null`, and `Demand::Current`
   is unrefusable. Past the cut-off it is in territory no golden covers and where the `c_e`
   floor is running 19–21 Newton iterations. Finite and monotone → a mark past it with a
   named caveat. Anything else → the mark sits before it.
4. **Whether the two arms' `soc_true` traces coincide.** Both are stoichiometry-derived
   rather than coulomb-counted — `dfn::raw_soc` is the negative electrode's bulk stoich
   averaged across x — so "the same charge removed" is an assumption, not an identity.
   Verify before writing it. In particular **"three-fifths of the charge still in it" is
   coulomb arithmetic** (1.993 of 5.153198) and the page's readout is not: either quote
   the readout or say which of the two quantities the sentence means.
5. **Whether the committed replay numbers transfer to a free run.** The 3.42 V / 2.59 V
   row is batsim's SPM against *PyBaMM's DFN* on the **reference's clock**. The page shows
   batsim's SPM against batsim's DFN, free-running, at `dt = 2`. A constant 3C makes the
   demand identical so they very likely agree — and "very likely" is precisely what the
   three prior client slices were burned on. No voltage from that table enters an `expect`
   block until the free-running trace has been read at the shared mark.
6. **Cost in wasm.** ~180 µs per cell per step native at the recommended grid, ×2 solve
   passes at 1S1P; the browser number has never been taken. It decides `speed_x`.
7. **The trajectory at the page's `dt`.** See below — this is the one real design
   decision.

## The design decision: run the configuration the golden asserts

The value of this slice is a specific honesty: *the DFN arm's numbers are backed by a
committed golden; the SPM arm's are not.* That only holds if the scenario ships **the
configuration the golden runs**. So the DFN file is pinned to `dfn_golden.rs` rather than
to anything more accurate:

* grid **10/5/10** (`DEFAULT_NODES_*`) and **20 shells** (`DEFAULT_SHELLS`);
* `initial_soc = 1.0`, `initial_temp_k = 298.15`, `ThermalConfig::Isothermal`, `seed` 0,
  no scatter, no BMS, no aging, no faults.

10/5/10 is **known to be coarse at exactly this rate**: 464 s against the reference's
488.7 s, 5.1 % short, where 20/10/20 gives 490 s and 21.8 mV instead of 62.1. Shipping the
finer grid would be a better number with no reference behind it. Shipping the coarse one
and *writing the 5.1 % into the file comment* is the stronger document, and it is also the
honest one: `refining_the_x_grid_converges_toward_the_reference` exists precisely so that
price is a committed number rather than a sentence.

**The timestep is the one thing that must move.** `run_to_cutoff` uses `dt = 2.0 s` at 3C;
the page's box defaults to 0.5. A lesson that leaves it there runs a trajectory the golden
does not cover. `applyStep` already reads an optional `L.dt` (`web/app.js:2836`) and sets
the box, so **both lessons set `dt: 2` and no page change is required** — the field exists
and is unused by every current record. Measure the DFN at 0.5 s and at 2.0 s anyway: if
they agree to a few millivolts the file comment says so, and a reader who changes the box
is not silently off-golden.

### The consequence nobody has had to think about before: `dt` persists

These are the **first lessons in the path to set `dt` at all**, so the field's persistence
has never mattered. It matters now: `applyStep` sets the box only
`if (L.dt !== undefined)`, and its own comment says every other step deliberately "leaves
the box where the reader left it". So `dt = 2` leaks **in both directions**:

* forward into steps 17 and 18 (the external-short pair, which this slice pushes down two
  places) — and step 17's `expect` quotes `t = 133.5 s`, `t = 156.0 s`, `39.62 %` and
  `93.29 A on the first frame after the fault`, every one of them step-resolution
  dependent and none of them measured at 2 s;
* backward into step 14, whose ×1.87 / ×6.01 decomposition was measured at the page's
  default.

**Fix: steps 14, 17 and 18 gain an explicit `dt` pinning what they were measured at.**
That is three one-line additions and no behaviour change for a reader who walks the path
in order from a fresh load — but it is what stops a mark set here from silently rewriting
four committed numbers two steps later. Confirm during implementation that nothing else
writes the box (`loadScenario` in particular), and re-read steps 14, 17 and 18's numbers
off the page after the change rather than trusting that pinning restored them.

## Part A — the two scenario files

`scenarios/cc_discharge_3c_spm.toml` and `scenarios/cc_discharge_3c_dfn.toml`, differing
in **exactly one block**, which is the `pulse_train_ecm` / `pulse_train_spm` idiom and the
thing that makes what a reader sees attributable to the model rather than the setup:

```toml
[pack.cell_model.Dfn]
shells = 20
nodes_negative = 10
nodes_separator = 5
nodes_positive = 10
```

Both name `chemistry = "nmc_21700_lgm50"` — the only file with both an `[spm]` and a
`[dfn]` section, and `Pack::new` names whichever is missing, so no other chemistry can host
this scenario.

Naming: rate-and-model rather than chemistry, unlike the three `cc_discharge_<chem>` files,
because the chemistry is fixed and the rate and the model are what vary. `depletion_3c_*`
was considered and rejected — it names the conclusion rather than the experiment.

Each file's comment carries, in the house shape: which half of the chemistry file it reads
(the `[spm]` electrodes plus the `[dfn]` electrolyte, both extracted from Chen2020, neither
fitted — so unlike `cc_discharge_lgm50.toml` this pair reads **no placeholder**, and its
millivolts are comparable in a way the ECM arm's never were); what the grid costs; what it
shows; and the limitation, which is the run past cut-off.

**The SPM file's comment states its own asymmetry**: its arm has no 3C golden. That is a
sentence, not a defect, and writing it is what this repo does.

## Part B — two lessons, inserted at 15 and 16

The path is 16 steps. The model arc is steps 12–14 (`pulse_train_ecm` → `pulse_train_spm`
→ the same at 3C), and step 14 ends on *"no single resistance can be 1.87 and 6.01 at
once, which is the whole argument for a model with an inside"*. The natural next sentence
is that a cell also has an **outside** — the electrolyte between the particles — so the two
new lessons go directly after it, making the external-short pair 17 and 18.

**This renumbers nothing that is referenced.** Prose and comments cite steps 1, 2, 5, 10,
11, 12 and 13 — all before the insertion point — and the two lessons that move cite only
steps 5, 10 and 11. Verify with a grep before and after; do not trust this paragraph.

**Both steps share one mark, and the mark is a measurement.** The comparison is "the same
cell at the same current at the same instant", so a step that runs to the SPM's own
cut-off and a step that runs to the DFN's would put the reader in front of two different
x-axes and the effect would be invisible. One `until_s`, used by both, chosen from
measurement item 3 — what the DFN does after 464 s decides whether the mark sits past its
cut-off (with the caveat named, the way `pulse_train_spm.toml` names its own) or before it.

The consequence is load-bearing and must be written into step 15's prose rather than
discovered by the reader: **inside that window the SPM never reaches a cut-off at all.**
It is still somewhere around 3.4 V with most of its charge showing when the window ends.
Step 15 therefore cannot promise a cut-off, and should not try to.

* **Step 15 — the SPM at 3C.** `cc_discharge_3c_spm.toml`, `Current` 15.459594 A, 25 °C,
  no BMS, `dt: 2`, `reload: true` (the mark does not ascend from step 14's 1980 s).
  Watch `plot-v`, `plot-soc`. The point of this step is that **the answer looks fine**: a
  smooth curve, an unremarkable slope, nothing anomalous to see. A reader has no way to
  know it is wrong from the trace alone. That is the setup, and it must not be spoiled.
* **Step 16 — the same discharge, one field different.** `cc_discharge_3c_dfn.toml`, same
  demand, same `dt`, same mark, `reload: true`. The cell is finished in roughly *half* the
  time the SPM would need — the whole gap visible inside one window — at
  ~1.99 A·h of 5.15 — three-fifths of the charge still in it — because the electrolyte
  between the particles has been drained faster than it can be replenished, and κ collapses
  with it. Then the instruction that makes it a lesson rather than a demonstration:
  **drop the current to 5.153 A and run both again.** The gap very nearly closes. The
  reader has just located a regime boundary, which is worth more than either trace.

The `expect` blocks are written **after** the measurement, from the page, at the page's
`dt`, following the SPM lessons' decomposition style.

## Part C — the prose sweep, by capability not by literal

The capability that changes is **"a client can select the DFN"**. Grepping `dfn` would
miss most of it (balancing-chatter's lesson: three literal numbers missed two of four
files). Candidates to read and edit, each because of what it *claims* rather than what it
spells:

* `scenarios/cc_discharge_lgm50.toml` — its "What it deliberately does not do" block, which
  discusses model reachability and was already edited once by the SPM slice;
* `scenarios/pulse_train_spm.toml` — "until this file no client could reach it", now true
  of one model and not the other;
* `crates/sim-core/src/dfn.rs` module doc and `CellModelConfig::Dfn`'s doc — anything
  claiming no client reaches it, and the cost note, which a scenario now makes concrete;
* `README.md` — the scenario catalogue prose and any model/client table;
* `docs/plans/dfn-aging-gap.md` — the "Still open" bullet this slice closes;
* `docs/plans/spm-scenario.md` and `docs/plans/phase-7-dfn.md` — anything deferring this.

And one that is not about reachability at all, which is why a capability sweep finds it
and a literal one does not: **lesson 14's own `expect` ends with "do not run this one to
empty: past the charge clamp the particle model pins near 0.4 V"** — and step 15 then runs
that same model at 3C toward empty, on the next screen. Whether it actually reaches the
clamp is measurement item 2 (at ~13 % SOC at cut-off it probably does not), but *reads as
contradicting the previous step* is a defect independent of the answer. Either step 15
says why its run is inside the warning or step 14's warning is qualified. Decide it from
the measurement, and do not leave the reader to reconcile the two.

## Verification

1. `cargo test --workspace` — `every_shipped_scenario_parses_builds_and_steps` picks both
   files up automatically and costs one resting step each. `cargo clippy --workspace
   --all-targets -- -D warnings`, `cargo fmt --all`.
2. **Drive the page.** Headless Chrome over CDP (the protection-escalation recipe: `PUT
   /json/new`, one debugger session per target, IIFE-wrapped evaluates, pace the poll loop
   or it starves rAF). Every number in every `expect` block is read off the page, never off
   a harness — a `Frame`'s `sim_time_s` is read *after* its step, so the harness and the
   page disagree by one.
3. **Walk the path in both directions**, and read steps 14, 17 and 18's numbers off the
   page on the way past in each direction. A forward-only walk proves nothing about
   `path-back`; the reload rule is an inequality that holds one way, which is why both new
   steps carry `reload: true` and why Back must be exercised. The `dt` pins are the thing
   this walk is really testing — a walk that only checks the two new steps would pass over
   exactly the damage they can do.
4. Confirm the picker lists both files with no page edit (`GET /scenarios` fills it), and
   that the per-cell grid and readouts render a DFN pack without special-casing.
5. Rebuild `web/pkg` if anything Rust-side moves. Nothing should.

## Deferred, with a price

* **A 3C SPM golden — declined, not overlooked.** It would make both arms
  reference-backed, and it costs a PyBaMM run in `tools/reference/` plus a committed CSV
  plus a test. Declined because this is a client-reachability slice and the asymmetry is
  one sentence in a file comment; stating it is cheaper than removing it and is what the
  repo does elsewhere. Recorded here so a later reader sees a decision rather than a gap.
* **A DFN *pack* scenario.** ~180 µs per cell per step and a topology-dependent pass count
  puts 10S10P at ~18 ms/step — a fast-forward, not a study. 1S1P is the honest client
  configuration and the only one this slice ships.
* **Moving `DEFAULT_NODES_*`.** Slice D measured it and the owner's call was to record
  rather than move. Nothing here re-opens that.

## Versions

Expected to move: **nothing**. `SNAPSHOT_VERSION` 13, `API_VERSION` 2, `WASM_API_VERSION`
4 — two TOML files, two JavaScript records, and prose. Each constant's own doc gets read
individually anyway; that check has caught a parted pair before (`ui-bms-view`).

---

# What the measurement said, and the four places it moved the plan

Everything above is the plan as drafted. This section is what happened when the seven
unmeasured items were measured, written before a word of `expect` prose. The harness is a
scratch bin crate outside the repo (path deps on `sim-core`/`sim-data`), not a test —
nothing here is asserted, and what deserved an assertion went into the scenario comments
and `NEWTON_ITER_CAP`'s doc instead.

## The table

All 1S1P, isothermal, 25 °C, no BMS, from 100 % SOC, `nmc_21700_lgm50`, 3C = 15.459594 A,
grid 10/5/10 with 20 shells.

| quantity | DFN | SPM |
| -------- | --- | --- |
| cut-off at `dt = 2` | **464.0 s, 1.9926 A·h, 61.33 % showing** | **1060.0 s, 4.5520 A·h, 11.67 % showing** |
| cut-off at `dt = 0.5` | 463.5 s, 1.9904 A·h | 1059.0 s, 4.5477 A·h |
| worst \|Δv\| between the two `dt` | 3.2 mV to 440 s, 10.1 mV to 460 s, 17.5 mV at 462 s | **0.57 mV, whole run** |
| flags before its own cut-off | **none** | none |
| first `SOLVE_UNCONVERGED` | **466 s — one step after the cut-off** | never |
| converged step cost | **360 µs** (= the committed 180 µs/cell × 2 passes) | 1.7 µs |
| unconverged step cost | **8177 µs — 23×** | n/a |

And the pair, which is the lesson:

| rate | DFN cut-off | SPM cut-off | disagreement | mean \|Δv\| to the DFN's cut-off | worst |
| ---- | ----------- | ----------- | ------------ | ------------------------------- | ----- |
| 1C | 3484 s | 3496 s | **0.34 %** | 60.1 mV | 71.3 mV |
| 3C | 464 s | 1060 s | **128 %** | 401.4 mV | 1014.8 mV |

## Item by item

1. **batsim's own SPM at 3C.** 1060 s / 4.5520 A·h, no flag on any step. (PyBaMM's SPM
   answers 1084.7 s on its own 5.0 A·h basis; different capacity bases, so the two are not
   a comparison and the file comment says so.)
2. **Does the SPM arm fall off its OCP table before 2.5 V?** **No** — it reaches the
   cut-off at 11.67 % SOC and only pins (near **0.31 V**, not the 0.4 V step 14 quotes
   from a different starting point) past ~1160 s, once the SOC clamp is reached. So step
   15's run is *outside* step 14's warning, and both texts now say so: step 14's tail was
   rewritten from "do not run this one to empty" — which step 15 would have read as
   contradicting on the next screen — to a statement about the SOC clamp specifically,
   with a forward reference. The plan asked for this to be decided from the measurement
   rather than left to the reader; it was.
3. **What the DFN does after 464 s.** Finite to a 1200 s horizon, but **not monotone**
   (drifts *up* by as much as 10.4 mV at `dt = 2`) and `SOLVE_UNCONVERGED` from 466 s on.
   The plan's rule — "finite and monotone → a mark past it; anything else → before it" —
   was written without knowing the cost, and the cost is what actually decided it: see
   below.
4. **Do the two arms' `soc_true` traces coincide?** **Yes, to 3.9e-15** — bit-for-bit for
   practical purposes, at every sample. Both readouts are exactly linear in charge removed,
   so the plan's worry that "three-fifths of the charge still in it" is coulomb arithmetic
   while the readout is not **dissolves**: 1.9926 / 5.153198 = 38.67 % delivered, and the
   readout says 61.33 %. They are the same number. Verified rather than assumed, which was
   the point.
5. **Do the committed replay numbers transfer to a free run?** Not quoted, so it does not
   arise. The 3.42 V / 2.59 V row is a replay against PyBaMM's clock; the free-running
   answer at the shared instant is **2.4218 V (DFN) against 3.4366 V (SPM)**, and that is
   what the prose quotes. No voltage from the golden's table entered an `expect` block.
6. **Cost in wasm.** Not the binding constraint at 1S1P and not separately measured: a
   500 s run is 232 ms of native arithmetic, which at `speed_x = 100` is ~4.6 % of a core
   natively and comfortable in wasm. What *was* the binding constraint is item 3's price.
7. **The trajectory at the page's `dt`.** Both lessons set `dt: 2` as planned. The DFN arm
   is genuinely sensitive across the knee (17.5 mV at 462 s) and its file comment says so;
   the SPM arm is indifferent (0.57 mV).

## The four changes

**1. The mark is 500 s, not "past the cut-off with a caveat" or "before it".** The
deciding measurement is one the plan did not anticipate: **an unconverged DFN step costs
23× a converged one** (8177 µs against 360). Past 464 s every step runs `NEWTON_ITER_CAP`
out. A 600 s mark spends 556 ms of arithmetic on 136 s of flat line — more than twice the
whole supported run — and at `speed_x = 100` that is ~40 % of a native core and likely
more than a wasm one has. 500 s contains the entire supported trajectory, the knee, and
36 s of aftermath (18 unconverged steps, 148 ms) which is enough to show the trace has
flatlined rather than dipped. The excursion is named in both the file comment and the
lesson, including the tell that it is an artefact: the shelf near 2.38 V drifts *upward*.

This is also now recorded in `dfn::NEWTON_ITER_CAP`'s own doc, because it is a property of
the solver rather than of this slice, and nobody had priced it.

**2. The `dt` pins go on steps 12, 13 and 14 — not 14, 17 and 18.** The plan's forward-leak
analysis was stale: steps 17 and 18 have carried `dt: 0.5` since the protection-escalation
slice, so the forward direction was already blocked and no edit was owed there. The
*backward* leak is the live one and it is worse than the plan says — `dt = 2` set at step
15 leaks back through 14 → 13 → 12, and **12 and 13 had no pin either**, so the 74.8 mV
and 17.3 → 37.2 mV rebound decompositions were exposed. Three one-line additions, as
predicted; a different three. Note the failure mode is not "the legs break": at `dt = 2`
the 60/600 s pulse legs are still whole steps (30 and 300), so nothing would have thrown —
the numbers would simply have stopped matching, silently. That is why this needs a walk in
both directions rather than an assertion.

**3. The renumbering grep found nothing, in both spellings.** Every digit citation in
`web/app.js` points at steps 1, 2, 5, 10, 11, 12, 13 and 14 — all before the insertion
point. The spelled-out sweep found three counts ("Eight steps of taking charge out", "none
of them have appeared in eight steps", "Eleven steps of protection have derated a demand")
and all three are at steps ≤ 14 and count *protection* steps rather than path positions, so
the two insertions — both `bms: null` — leave them true. `README.md`'s "sixteen steps" was
the one real hit and is now eighteen. `docs/plans/spm-scenario.md` was on the candidate list
and turned out to defer nothing about the DFN; that is a result, not an omission.

**4. One defect the plan did not know about.** `$("path-exit")` relabelled the start button
`"Start — 8 steps"` — stale by four insertions, and `index.html` said 16, so the two had
already drifted. Both now derive from `LESSONS.length`; the markup string is only what
shows before the script runs.

## What the measurement did *not* change

The design decision stands exactly as drafted: the DFN file ships **the golden's
configuration**, 10/5/10 and 20 shells, with the 5.1 % written into its comment. The
1C-versus-3C table is the strongest argument for it — a reader who wants to know whether
the coarse grid is costing them something has a committed test that says what refining buys.

## What driving the page found

Both `expect` blocks were written **after** this, from the page, and every number in them
is one of the readings below. The harness is headless Chrome over CDP in
`M:\claud_projects\temp`; the page is at `/app/`, not `/`.

**The panel's precision is not the engine's, and the prose had to be rewritten for it.**
`#readouts` prints terminal voltage to **three** decimals and SOC to **one** — so the
drafted "3.9180 V" and "58.33 %" were quoting digits the reader cannot see. Worse,
`fmtTime` renders anything from 120 s to 7200 s as whole minutes, so **464 s and 500 s both
display as "8m"**: no time in either lesson is readable off that panel. Exact times come
from `GET /sessions` (`pack.sim_time_s`), which is why the sampling pass runs over the
server socket rather than the in-page engine.

Every instant either lesson quotes, read off the panel:

| t \[s\] | SPM V | SPM SOC | DFN V | DFN SOC | DFN flags |
| ------ | ----- | ------- | ----- | ------- | --------- |
| 0 (the `readNow` probe) | 3.927 | 100.0 % | **2.808** | 100.0 % | — |
| 2 | 3.918 | 99.8 % | 3.839 | 99.8 % | — |
| 400 | 3.471 | 66.7 % | 2.957 | 66.7 % | — |
| 440 | 3.449 | 63.3 % | 2.860 | 63.3 % | — |
| 462 | — | — | 2.638 | 61.5 % | — |
| **464** | **3.437** | 61.3 % | **2.422** | 61.3 % | — |
| **466** | — | — | 2.414 | 61.2 % | **`SOLVE_UNCONVERGED`** |
| 500 (the mark) | 3.418 | 58.3 % | 2.379 | 58.3 % | `SOLVE_UNCONVERGED` |
| 1058 | 2.502 | 11.8 % | — | — | — |
| **1060** | **2.495** | 11.7 % | — | — | — |

`SOLVE_UNCONVERGED` first appears at **exactly 466 s**, one step after the cut-off, as
predicted — and it **renders**, which was worth checking rather than assuming: the flags
panel is a hand-written renderer, but `parseFlags` splits whatever string the engine sends
(`"OV | PLATING_RISK"`) rather than consulting a name table, so a flag added in Phase 7
needs no page change. Confirmed on the screen, not by reading the renderer.

### One reading the plan did not predict: the DFN opens at 2.808 V

Arriving at step 16, the panel reads **2.808 V** before the reader presses anything, where
step 15's read 3.927. Not a defect and not a resting voltage: `applyStep` ends with a
zero-length `readNow()` *at the demand just dialled in*, so this is the instantaneous
response to 15.46 A with no time for anything to move. All three models were probed
directly to be sure — `Rest` at `dt = 0` gives 4.2017 V on both porous-electrode models and
4.2000 V on the circuit, and `Current(3C)` at `dt = 0` gives **3.798 V (ECM), 3.927 V (SPM),
2.808 V (DFN)**, reproducing the page exactly. It is a real difference between the models
and a reader will see it first, so step 16 now opens on it rather than leaving it to be
noticed and mistrusted.

### Heat is a second page-visible channel, and it was free

At the mark the DFN reads **22.41 W** against the SPM's **6.33 W** from the identical
current. `q_gen` is the current times the gap between equilibrium and the terminal, which
is exactly where the disagreement lives, so it is the same finding in a second column.

### The `dt` pins, both directions

The point of the pins is the **back** walk, and it is what was measured:

| step | forward | back |
| ---- | ------- | ---- |
| 12 `pulse_train_ecm` | `dt=0.5`, 4.052 V, 81.7 % | `dt=0.5`, 4.052 V, 81.7 % |
| 13 `pulse_train_spm` | `dt=0.5`, 4.055 V, 81.7 % | `dt=0.5`, 4.055 V, 81.7 % |
| 14 3C pulses | `dt=0.5`, 3.989 V, 75.0 % | `dt=0.5`, 3.989 V, 75.0 % |
| 15 SPM 3C | `dt=2`, 3.418 V, 58.3 % | `dt=2`, 3.418 V, 58.3 % |
| 16 DFN 3C | `dt=2`, 2.379 V, 58.3 %, `SOLVE_UNCONVERGED` | — |
| 17 external short | `dt=0.5`, 89.4 %, `CONTACTOR_OPEN` | — |

Every step reads **identically in both directions**. Without the pins, 14, 13 and 12 would
have inherited `dt = 2` from step 15 on the way back and quietly re-run at four times their
measured step length — the legs still divide (60/600 into 2 s), so nothing would have
thrown; the millivolts would simply have stopped matching.

### Two harness findings worth keeping

* **A readiness probe must not match its own static fallback.** The load check waited for
  the start button to read "… steps" — which is exactly the string this slice put into
  `index.html` as the pre-script fallback. It matched before `app.js` had bound a single
  handler, so the first `.click()` landed on a button with no `onclick` and did nothing, and
  the walk sat waiting on a path that had never started. The fix is to wait for something
  only the module can produce; here, the scenario `<select>` filled from `GET /scenarios`.
* **In `--headless=new`, `requestAnimationFrame` does not fire on its own**, so the page
  arms a run and never advances. `Page.captureScreenshot` drives exactly one frame — the
  screenshot is the clock, not the observation. And `stepsForFrame` caps a frame's wall
  delta at 0.25 s, so one forced frame buys `0.25 × speed_x` of simulation and no more,
  which is what makes a full 18-step walk expensive enough to be worth skipping through.

**Not verified, and said rather than glossed:** step 18 (`nothing-to-clamp`) hung this
harness on arrival, in both directions, twice — under a browser that had accumulated a
dozen live pages and again on a clean one. Nothing in this slice touches that step: its
`dt: 0.5` pin predates it, and step 17 was confirmed forward at `dt = 0.5` immediately
after step 16 set the box to 2, which is the only interaction this slice could have had
with either. Left as a harness limitation rather than claimed as a pass.

## The number that turned out to be the lesson

The plan expected the headline to be the voltage gap. It is not. It is the **cut-off**:

> At 1C the two models disagree about when the cell is empty by **12 seconds in 3484**.
> At 3C they disagree by **596 seconds in 464**.

The mean voltage gap does *not* vanish at 1C — 60.1 mV of standing offset survives, and
the file comment says so rather than rounding it away. What collapses is the disagreement
about the one quantity anyone designs against. That is a sharper statement of "a cliff
between 1C and 3C, not a slope" than the spike's millivolt table, and it is what step 16
closes on.

## Exit criterion

Both files load from the picker with no page edit; the guided path runs 18 steps forward
and back; and a reader who runs step 15 and then step 16 sees the same cell, at the same
current, die in half the time — with the reason on the screen rather than in the prose, and
with every number in the `expect` blocks read off that screen.
