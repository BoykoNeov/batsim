# Phase 8 slice B — wiring the LTO cell into the client, and teaching it

**Status: LANDED 2026-08-27.** Three scenario files, two guided-path steps, seventeen
claims, four arms and eighteen ledger rules. No engine code changed; the only Rust touched
is `crates/sim-data/tests/path_claims.rs`, which is a test.

Slice A (`phase-8-slice-a-lto.md`) shipped `chemistries/lto_20ah_generic.toml` with zero
engine code and closed the phase's first exit criterion. It closed nothing about the
phase's *second* criterion, which is that a chemistry is done **when a guided-path lesson
teaches it** — the owner's scoping decision, recorded in `phase-8-chemistries.md`. This
slice is that lesson, and it turned out to be two.

---

## What shipped

| file | what it is |
| ---- | ---------- |
| `scenarios/cc_discharge_lto.toml` | 1S1P LTO, full, 25 °C, isothermal, no BMS, no aging |
| `scenarios/cold_charge_lto.toml` | 1S1P LTO, 20 % charge, −25 °C, isothermal, **aging on** |
| `scenarios/cold_charge_nmc.toml` | the same file with the chemistry id changed |
| step 25, `ten-c-costs-a-point` | 200 A out of a 20 Ah cell, and what it costs |
| step 26, `cold-and-nothing-plates` | the same cold fast charge, and the flag that never comes |

Both steps are **ledgered** — every numeral in their prose is tied to something — so
`[ledger].unledgered` is still empty and the file's own "twenty-six steps of the twenty-six
so far" remains true. That was a deliberate choice and it is most of the slice's cost; the
alternative was to declare the two new steps unchecked in a list that exists for exactly
that purpose, and a phase whose point is that a chemistry is *taught* should not close by
adding the first two lessons nothing reads.

---

## The two lessons, and why these two

Slice A named them and priced them, and both survived contact with measurement:

**Step 25 — the rate.** Discharge the cell at its own rated 10 C and read what is left when
it declares itself empty. The frame is deliberately the one steps 22 to 24 use for
lead-acid — *what is still in the cell at the cut-off*, read off `soc (true)` — so the
comparison is a comparison and not a change of subject. Measured at dt = 0.5:

| cell | rate | cut-off | still showing |
| ---- | ---- | ------- | ------------- |
| LTO 20 Ah | 1 C (20 A) | 3595.5 s | 0.125 % |
| LTO 20 Ah | **10 C (200 A)** | **355.5 s** | **1.250 %** |
| NMC 18650 | 10 C (30 A) | 131.5 s | **63.472 %** |
| lead-acid AGM (step 23) | 3 C | 737 s | 38.6 % |

Ten times the current costs the LTO cell about one point of charge. The same multiple of
itself costs the graphite cell nearly two thirds of its charge. Both arms are on the page:
one is the demand box, the other is `cc_discharge_nmc.toml` out of the picker.

**Step 26 — the cold charge.** The path already teaches cold-charge plating, in step 11's
closing instruction: drag the ambient below freezing, switch the BMS off, watch
`PLATING_RISK`. This step gives an LTO cell a harsher version of the same abuse — −25 °C,
4 C, from a fifth full — and nothing happens at all. The control arm is the same scenario
file with the chemistry id changed, and it plates on the first step.

---

## Two things had to be decided before a word of prose was written

Both were flagged by review before the work started, and one of them would have made a
whole written lesson false.

### 1. A cold lesson cannot be built by dragging the ambient slider

Every `cc_discharge_*` scenario in this repo is **isothermal**, and `PLATING_RISK` is
judged on the *cell's* temperature (`pack.rs`, `plating_risk(safety, i_k, temp_before, …)`),
not the ambient. On an isothermal file the slider decides nothing about the cell, so the
obvious lesson — "drag the ambient down and watch what does not happen" — would have shown
the reader a control that changes no state at all. Step 11's cold arm works only because
*its* scenario carries `[pack.thermal.Network]` and really does cool.

So the cold lesson starts cold: `initial_temp_k = 248.15`, and the scenario file says why
it is not a cooling pack. The reason is attribution — a cooling cell changes its resistance
while you watch — and the cost is stated in the file rather than discovered later: at
22.28 W into a 510 J/K case an adiabatic version of this run would gain about 26 K over ten
minutes, and **a warming cell plates less**, so isothermal is the harsher reading for the
graphite control rather than a kind one.

### 2. The chemistry is reachable by both transports, and neither needed a rebuild

Slice A measured only the server route. The wasm route is `web/app.js`'s
`WasmBackend.create`, which asks Rust for the scenario's chemistry id and then fetches
`/chemistries/{id}.toml` from the same `ServeDir` — a directory lookup, not an embedded
table, so a new chemistry file is reachable in the browser the moment it lands and the
`web/pkg` build is untouched by this slice. Checked rather than assumed, because this repo
has been wrong about "reachable through the existing mechanism" before.

The **scenario** picker is the same shape — `loadScenarioList` fills it from
`GET /scenarios`, a directory scan — with one wrinkle that cost an HTML edit: the
`<option>` list in `web/index.html` is a *fallback* for a page newer than its server, and
`assert_picker` in the claims harness reads that fallback list when an arm says "load this
file from the picker". So `cold_charge_nmc.toml` is in the fallback list, and the other two
new scenarios are not, because no sentence sends a reader to them by name.

---

## What the ledger cost, and the three things it caught

Eighteen new `LEDGER_VOCABULARY` rules and seventeen claims. Three findings, all of them
from the check rather than from reading:

**The English ban is wider than the reader, exactly as advertised.** The first draft of
step 25 said the rate "costs this cell a point of its charge", and
`no_lesson_spells_a_quantity_in_english` refused it. That is the digits rule (slice 0)
working on the first prose written after it: a false alarm costs a rewritten sentence and
is visible immediately, which is the whole argument for banning rather than reading. The
sentence now names neither number and points at the two readings instead.

**A number can be claimed and configured at the same instant, and then it is neither.**
Step 26 originally said `PLATING_RISK` "is up on the very first step, at 0.5 s". That
`0.5` is also the step's **timestep**, so check 6 found two readings of one number and
refused both — an accounting decided by which arm the checker tried first is not an
accounting. On a page whose default step is 0.5 s there is no way to spell "the first
step" in digits that does not collide, so the sentence gives no instant and the claim is
`states = "nothing"` with a `grid` tolerance. Half a step is the tightest bound a flag
time admits anyway, so nothing was lost but the digit.

**A rule written for a new step matched an old one.** `the clock reads `{n}m`` was written
for step 25's `6m` and immediately collided with step 1's `— the clock reads `69m` —`,
where the number is already inside a claimed literal. Narrowing the phrase to `and the
clock reads `{n}m`` separated them. The double-cover panic is what found it, and it is the
same fence that has caught mis-pointed rules three slices running.

**Nineteen self-stated counts were stale by the end of it** — every `tol_from` tally, the
`states` tallies, the claimed-sentence counts, the ledger's own "twenty-four of
twenty-four", and the module doc's `24 teaching steps`. All nineteen are derived and
checked (`every_count_these_files_state_about_themselves_is_derived`), so all nineteen
failed loudly and were repaired from the test's own messages. This is the machinery from
`path-self-counts.md` doing precisely the job it was built for, on the first slice since
that added a step.

---

## Both steps were walked in a real browser, and every displayed number agreed

The claims harness is a 16 000-line mirror of the page, so it can be right about a
trajectory the page never shows. Both steps were therefore walked end to end in Chrome
against `sim-server`, and the panel was read at each mark:

| row | step 25 at 355.5 s | step 26 at 600 s |
| --- | ------------------ | ---------------- |
| `sim time` | `6m` | `10m` |
| `terminal` | `1.481 V` | `2.747 V` |
| `soc (true)` | `1.3 %` | `86.7 %` |
| `heat` | `55.89 W` | `22.28 W` |
| `soh cap` | `100.00 %` | `100.00 %` |
| flags | none | **`no flags`** |

Every one is the string its claim declares, and the last row is the lesson: at the mark the
flag column really is empty. Both steps also stop *at* their marks rather than past them,
which matters for step 26, where the first flag is three seconds later.

Two environment facts confirmed on the way, both already recorded in earlier slices and
both worth re-stating because they make a browser check look like a broken page:
`requestAnimationFrame` does not fire at all while the window is occluded — the run sits at
`0s` with the button reading **Pause** — and a screenshot forces a frame, so the sim
advances in the jumps between tool calls rather than continuously.

The picker and both transports were checked at the same time: `GET /scenarios` lists all
three new files with no parse error, and the page's wasm backend fetched
`/chemistries/lto_20ah_generic.toml` over HTTP with no rebuild of `web/pkg`.

---

## The gap this slice leaves, named rather than left to look like coverage

**No claim in `path-claims.toml` asserts that the LTO cell never plates.** There is no
"this flag never fires" quantity — `flag_first_s:<FLAG>` panics when the flag is absent,
and none of the three `tol_from` rules would fit a boolean. What stands behind that half of
step 26's sentence is
`crates/sim-data/tests/lto_chemistry.rs::cold_fast_charge_plates_the_nmc_cell_and_not_the_lto_cell`,
which drives both cells at 4 C at −30 °C and asserts the absence directly. That test
predates this slice and is a hand-built pack rather than the page's trajectory, so the two
are not the same statement; the claim on the `run-on` arm says what *does* happen (the
terminal crosses its ceiling at 603.0 s, a voltage limit and not a plating one) and its
note records that the absence is asserted elsewhere.

**And the axis under that one: a comparative sentence with no numeral in it is read by
nothing.** Two of the sentences this slice shipped were false, and both were found by
review after the suite was green, because neither contains a digit for the ledger to scan
or an English quantity for the ban to refuse:

* *"rates it at 10 C in both directions, an order of magnitude above anything else in this
  path"* — true on the charge side (10 against LFP's 1.0) and **false on discharge**, which
  is the direction the lesson is actually about: `max_discharge_c` is 3.0 on both LFP and
  lead-acid, so the factor is 3.3. Slice A had attached that phrase to `max_charge_c` alone
  and this slice widened it without re-reading the field.
* *"Aging is on, which no other constant-current lesson in this path switches on"* — false:
  step 21 runs `over_discharge_damage_lfp.toml` on a `Current` demand with `[pack.aging]`,
  and `soh cap` moving is its entire subject.

Both are repaired, and the repair for the second one turned up neighbour rot it did not
cause: `over_discharge_damage_lfp.toml`'s own description says *"UNLIKE every other
scenario here, aging is ON"*, which `calendar_fade_hot.toml`, both external shorts and the
soft short had already falsified before this slice arrived. Narrowed to the CC discharge
scenarios, where it is true.

The lesson generalises past these two: **the ledger closes the digit axis on a step and
says nothing about its comparatives.** "an order of magnitude above", "no other lesson
does", "the only file that", "nothing else in this path" — every one of those is a claim
about the repo that ages exactly the way a number does, and this file has now been wrong
about two of them in one slice. That is the cheapest target the path verification work has
left, and it is a *different* instrument from the digit scanner rather than an extension
of it.

One smaller thing of the same shape, named rather than fixed: step 25 closes with *"Take
the load off that NMC cell and it is an ordinary, mostly full cell again"* — an instruction
to the reader with no arm and no claim behind it. It is true of an ECM with no diffusion
state, and it prints no number, so nothing here reaches it.

Adding a `flag_never:` quantity is a slice of its own — it needs a fourth `tol_from`, and
that is a change to the tolerance taxonomy rather than an entry in it.

---

## What this slice did not do

* **No engine code.** Exit criterion 3 (the floor did not move) is structural here: three
  new scenario files that no existing pack loads, two new lesson blocks, and a test file.
  Nothing an existing trajectory reads changed.
* **No charge leg on the rate lesson**, and no discharge on the cold one. Each step is one
  demand and its arms, which is what makes anything a reader sees attributable.
* **No thermal coupling.** Both LTO scenarios are isothermal, for the reason above, and
  both files say what the coupled version would look like.
* **The two remaining shipped chemistry files are still outside the parameter-band tests**
  (`nmc_21700_lgm50.toml`, `pba_agm_2v_generic.toml`) — slice A declared that gap and this
  slice did not close it.

## What this prices for slice C

Slice C is the hysteresis state and NiMH's parameters, and it carries the phase's one
snapshot bump. Two things this slice measured that it will want:

1. **A cold isothermal scenario is cheap and teaches well.** The pattern here — two files
   differing in the chemistry id and nothing else, one lesson, one picker arm — is the
   cheapest reachable contrast in the repo, and it is the shape NiMH's end-of-charge lesson
   should take against a lithium control.
2. **Aging on a lesson scenario is what makes a flag cost something.** Step 26's whole
   payoff is `soh cap` reading `100.00 %` on one cell and `99.69 %` on the other at the
   same charge. Slice D's falling-voltage lesson has the same problem — a shape on a plot
   is weaker than a number on a row — and the same answer is available.
