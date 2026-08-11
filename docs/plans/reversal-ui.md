# Over-discharge, made visible — and four lessons that still described the old engine

`docs/plans/low-clamp-reversal.md` closed with this, under "Deferred, with a price":

> **No client shows the deficit yet.** `CellView::soc_deficit` is on the wire (see the
> versions table above for why that stopped being deferrable), but `web/app.js` neither
> reads it nor offers it as a pack-grid metric, and the guided path has no step about
> reversal. That is a UI slice, and it is now cheap: the data is already there.

It was cheap. It was also incomplete as a description of the work, and the missing half
is the larger one: **the reversal branch silently falsified prose in four of the guided
path's nineteen steps**, and every one of them was green under `cargo test --workspace`
because none of the path's numbers is an assertion anywhere.

So this slice is two things that had to happen together:

1. the page reads `soc_deficit` — a readout row, a cell-detail line, a pack-grid metric,
   and a twentieth lesson whose subject *is* over-discharge;
2. every lesson that drives a pack past empty is re-measured, and the four that were
   wrong are corrected.

## The instrument

`M:\claud_projects\temp\reversal-numbers` — a copy of the `path-numbers` harness
(`docs/plans/path-numbers.md` describes it) with `soc_deficit` added to its per-step
row and a `reversal` module for the five runs this slice needed. Same arrangement as
before: an out-of-tree binary with path dependencies on `sim-core` and `sim-data`,
reading the repo's own `scenarios/` and `chemistries/`, so it cannot drift from them.

**The audit was not done by reading.** A second copy, `prerev-numbers`, points its path
dependencies and its `REPO` constant at a `git worktree` of `7103306` — the commit
before the reversal branch landed — and both were run over the *whole* nineteen-step
table. Diffing the two outputs is the audit: anything the reversal changed shows up as
a changed line, and nothing that it did not change can hide in one.

That is the method this repo's memory asks for — hunt falsified prose by *capability*,
not by literal — mechanised, so the capability does not have to be guessed at. It found
one step that reading had not: step 14, whose claim about the circuit's floor is a
sentence about a *different scenario* than the one the step loads.

Diff, in full, with the three perf-timing lines dropped as machine noise:

| step | quantity | before | after |
| --- | --- | --- | --- |
| 1 | terminal at the 4200 s mark | 1.9360 V | **0.6387 V** |
| 2 | terminal at the 4200 s mark | 2.9168 V | **1.2501 V** |
| 7 | terminal at the 450 s mark | 5.7381 V | **−2.2619 V** |
| 7 | lowest cell over the run | 1.4267 V | **−0.5733 V** |
| 19 | terminal at the 300 s mark, BMS off | 5.0844 V | **0.1916 V** |
| 19 | current at that mark | 50.86 A | **1.95 A** |
| 19 | hottest cell at that mark | 108.73 °C | **98.08 °C** |
| 19 | lowest group voltage, BMS off | 1.1350 V | **−0.5549 V** |

Every `i_rejected_a` reading on those rows also went from a non-zero number to exactly
zero, which is the branch's whole point: charge drawn past empty is no longer invented
and then reported as invented, it is carried as a deficit and repaid.

## What the four lessons said, and what they say now

### Step 2 — a voltage that is 1.67 V too high

> …it finishes at **2.92 V** against a 3.00 V cutoff with no complaint from anything but
> the coulomb counter.

Measured: the cell empties at 4154.0 s and the mark is 4200 s, so it spends 46 s past
empty at 2.6 A — 1.11 % of its capacity. NMC's `[reversal]` slope is 150 V per unit
SOC, so that is 1.67 V of collapse, and the step now ends at **1.2501 V**. The rest of
the sentence survives: nothing complains but the coulomb counter.

### Step 7 — "dives well under 2.0 V" is now an understatement

> Cell voltage dives well under the 2.0 V the datasheet allows, temperature climbs, and
> the only flag you will see is `SOC_CLAMPED_LOW`.

Still true, and it now stops well short of what happens. Measured on the 4S2P pack at
40 A with no BMS: the first cell passes empty at 345.0 s, the last at 356.5 s — an
11.5 s spread that **is** the capacity scatter, made visible for the first time — the
lowest cell crosses 0 V at 351.0 s, and the *pack terminal* crosses zero at 358.5 s.
At the 450 s mark the pack reads **−2.2619 V** with its cells between −0.5733 and
−0.5571 V and their deficits between 22.31 and 26.18 points of capacity.

A pack sourcing a negative voltage into a 40 A load is the model saying, correctly, that
the load is now *driving* the pack. That is the physical content of reversal and it is
worth a sentence rather than a silent number change.

### Step 14 — a comparison whose other half no longer exists

> …run it past the **SOC clamp** — not to its cut-off, past empty — and it pins somewhere
> between 0.3 and 0.5 V while **the circuit stops at 1.79**, because the surface
> concentration falls off the bottom of its table.

The particle half is right: measured on `pulse_train_spm.toml` carried to 20 000 s, the
SPM settles at **0.3095 V** under the 3 C pulse and stays there.

The circuit half is now wrong twice over, and this is the claim reading had missed —
"the circuit" is `pulse_train_ecm.toml`, which is step *12*'s scenario, not this step's,
so no amount of running step 14 to its own mark could have caught it.

* On the pre-reversal engine it did not stop at 1.79 either. Measured on the worktree:
  the lowest terminal over a 20 000 s run is **1.8428 V**, 53 mV above the claim. A
  pre-existing inaccuracy, small, and inherited rather than introduced here.
* On this engine it does not stop at all. It clamps at 11 880.5 s — 600 s *later* than
  the SPM's 11 280.0 s, which is one whole off-leg and which the corrected prose now
  says rather than calling them the same moment — falls through zero, and repeats
  **−0.4508 V** at the start of each pulse and **−0.6572 V** by the end of one. The
  second number is the run's minimum and is the one a reader watching the plot sees;
  both are quoted, because the sampling that produces only the first is exactly the
  mistake this slice's first draft made. The floor is `floor_v` from the chemistry file,
  not an artifact.

So the sentence's whole rhetorical shape — *the particle has a hole in it, the circuit
does not* — has inverted. The circuit is now the one that keeps going, and it keeps
going *because* the model was fixed, not because it is unmodelled. The SPM's floor is
still a hole in a table; that half stays.

### Step 19 — the caveat that named the defect this slice's predecessor removed

> …`SOC_CLAMPED_LOW` at 235.5 s and 375 K, still climbing. Read that tail with one
> caveat, honestly: past the clamp the current does not stop, because this engine models
> no empty electrode and a cell at SOC 0 keeps sourcing at `OCV(0)` forever. It is the
> discharge face of the same hole step 10 shows on the charging side, and the heat after
> that is the model's, not the pack's.

Three separate falsehoods in one paragraph, and the load-bearing one is the last
sentence of the path's last step:

1. **"the current does not stop"** — it stops. Measured: 58.02 A at the clamp, 40.19 A
   4.5 s later, 17.04 A at 250 s, 1.95 A at 300 s, 0.098 A at 400 s. The short is still
   connected; the cells simply have nothing left to push through it.
2. **"375 K, still climbing"** — 375.565 K at the clamp is right, and it peaks at
   **376.341 K at 245.5 s** and falls from there: 371.2 K at 300 s, 362.2 K at 400 s.
   Under the old engine it was at 381.9 K at 300 s and rising, which is what "still
   climbing" was written from.
3. **"the same hole step 10 shows on the charging side"** — step 10's hole was closed by
   `docs/plans/energy-hole.md`, and step 10's own prose has said so since ("the charge
   that no longer fits is now **refused and burnt**, which it did not used to be"). The
   cross-reference was stale before this slice began.

The protected arm of step 19 needs nothing: it latches at 156.0 s at 39.62 %, which is
nowhere near empty.

### Step 1 — correct, and now the natural place to point forward from

Step 1 quotes no voltage at its mark, so nothing there is wrong. It does now end 53.5 s
past empty at 0.6387 V with a deficit of 1.297 points, which is the first time in the
path that a reader can see the new behaviour — and the twentieth step is that same
scenario, so a forward pointer is cheap and earns its line.

## What the page now reads

`soc_deficit` reaches the page in three places, chosen so that each answers a question
the other two cannot.

**A readout row, `past empty`.** The largest deficit over the pack's cells, in points of
state of charge, which is the unit `soc (true)` two rows above it prints. It is quiet on
every ordinary step, on the same terms as the `clamp` row that `energy-hole.md` added:
a row reading "0.000 pts" for a whole run teaches nothing.

This row is the reason `renderReadouts` grew one small capability. Its "nothing to
report" placeholder cannot be a constant, because the two silences mean different
things and the distinction is exactly the one this repo already drew for the surface-gap
row:

* on an equivalent-circuit pack, `none` — measured, and it never went past empty;
* on a porous-electrode pack, `particle model — no clamp` — there is no such quantity,
  because an `Spm`/`Dfn` cell has no SOC clamp to pass. Printing `none` there would be
  the "measured, and flat" lie `gapPts`'s own comment warns about.

So `READOUTS`' third element may now be a function of the same arguments the formatter
gets, and stays a plain string in all seven rows that had one.

**A cell-detail line**, printed only when the hovered cell's deficit is non-zero — the
same rule the internal-short and exotherm lines already follow, and for the same reason:
a row that is zero on every healthy cell is noise on every healthy cell.

**A pack-grid metric**, `soc_deficit`, gated to equivalent-circuit packs. The gate is
the interesting decision. `CellView::soc_deficit` is documented as a real `0.0` on a
porous cell rather than `null`, so the `avail` predicate cannot key on the field itself
the way the surface-gap metrics do. It keys on `surface_gap_neg === null` instead —
which is the wire's own answer to "is this an equivalent circuit" — because a metric
that paints every tile of a DFN pack identically reads as "the pack is uniform", and
`METRICS`' own doc comment calls that "a different and false claim".

On an ECM pack that has never been over-discharged the metric *is* flat, and that is
accepted rather than fixed: `internal_short_conductance_s` has shipped with exactly that
property since the pedagogy slice, and the tile numbers are printed, not merely coloured.

The metric earns its place on step 7's pack, where the eight cells pass empty across
11.5 s and the grid is the only view in which "which cell went first" is answerable.

## Step 20 — "Past empty, and the charge you have to put back"

`cc_discharge_lfp.toml`, the same single LFP cell step 1 opens the path with: 1S1P,
isothermal, no BMS, no aging, 2 A out. The advisor's argument for reusing it over step
19's shorted pack is that the short has temperature running away at the same time, which
muddies attribution for a step whose entire subject is a voltage.

Measured, and these are the step's numbers:

| moment | terminal | deficit |
| --- | --- | --- |
| empties (`SOC_CLAMPED_LOW`) at 4146.5 s | 1.9290 V | 0 |
| +30 s | 1.2055 V | 0.73 pts |
| +60 s | 0.4819 V | 1.45 pts |
| +80 s — crosses 0 V | −0.0004 V | 1.94 pts |
| +83 s — OCV reaches `floor_v` | −0.0640 V | 2.01 pts |
| +166 s | −0.0640 V | 4.01 pts |
| +253.5 s — the step's 4400 s mark | −0.0640 V | 6.121 pts |

The flat −0.0640 V is the whole shape of the model in one number: the open-circuit
voltage has reached the floor the chemistry file declares (`floor_v = 0.0`), so the
terminal is the ohmic drop alone and stops moving — while the deficit goes on growing
without bound, because charge is still being taken out. **A voltage that has stopped
falling is not a cell that has stopped discharging**, which is the lesson.

Then the half worth waiting for. Charging the same cell back at 2 A **from the step's own
mark**, where the deficit is 6.121 points (0.14099 A·h drawn past empty):

* `soc (true)` reads **0.0 % for 254 seconds** — four minutes in which every charge-state
  readout on the page is frozen and the charger is working the whole time;
* the terminal is no more informative: 0.026 V on the first step, 0.062 V a minute in
  and 0.064 V at two, because the cell is sitting on `floor_v` and the ohmic rise is the
  whole of it. It starts climbing at about 170 s, when the deficit re-enters the
  2-point ramp, and reaches 2.0655 V on the step the deficit clears;
* 254 s at 2 A is 0.1410 A·h, which is what was taken (0.14099 computed). **The debt is
  exact**, and that is the conservation property `low-clamp-reversal.md` bought, seen
  from the client.

**The first draft of this step got both of those wrong, and the browser pass is what
caught it.** The recharge had been measured from `t = 5000 s` — a run the harness had
carried past the mark — so the prose quoted a 20.59-point deficit and a 854-second
repayment that no reader stopping where the step stops could ever see. The engine numbers
were real; they were answers about a different moment. Same defect class as the two
"right but unreachable" claims `path-numbers.md` records, and it survived a plan doc, a
commit and every green gate, because nothing but walking the step can catch it.

`dt` is pinned at 0.5 s, which every step from 12 on does and which this one needs for a
reason of its own: step 18 tells the reader in so many words to put the box up to 5 s and
then 10 s, and `applyStep` leaves `dt` alone for a step that does not name one. An 83 s
collapse and a 854 s repayment measured at 0.5 s would both be quoted against whatever
the reader last typed.

The step is 20th and last rather than slotted next to step 10's overcharge, because it
is the answer to the question step 19 now ends by raising, and because the path's shape
is "here is an instrument" → "here is what it shows" → "here is where the model stops".

## What the browser pass confirmed

Driven headless over CDP against `sim-server` at `/app/`, reading the shipped page's own
DOM and clicking its own controls. Four things no Rust gate touches:

* **The function-valued `quiet`.** `past empty` reads `none` on `cc_discharge_lfp`, and
  `particle model — no clamp` on both `cc_discharge_3c_spm` and `cc_discharge_3c_dfn`,
  in the same dimmed class the other silent rows use.
* **The metric gate across a model change.** With `past empty` selected on the ECM pack
  (surface-gap options hidden), loading the SPM pack hides `soc_deficit` *and* returns
  the selector to `soc`. No exception out of a draw call; `window.__errs` empty
  throughout the whole session.
* **Step 20 at its mark**, walked from the start button through nineteen Nexts:
  `step 20 of 20`, terminal `-0.064 V`, `soc (true)` `0.0 %`, `past empty` `6.121 pts`.
* **The cell-detail line**, on hover: `… soc 0.000 % · … · past empty 6.121 pts · …`,
  and absent on every healthy pack read earlier in the same session.

Step 20's pack is 1S1P, so a second pass drove step 7's 4S2P setup — scenario loaded, BMS
unchecked, 40 A, `dt` 0.5 — and single-stepped across the crossing with the metric
selected. This is the case the 1S1P pass cannot reach: eight tiles, seven reading `0.000`
and one not, which is the ramp's `span > 0` branch and the legend's own min/max.

The tiles light up in this order, and it is the measured order:
`(0,0)`, `(0,1)`, `(3,0)`, `(3,1)`, `(2,1)`, `(2,0)`, `(1,0)`, `(1,1)` — first at 345.0 s,
last at 356.5 s, which is what step 7's corrected prose names. The legend reads
`0.000 pts → 2.856 pts` while part of the pack is still above empty and lifts its low end
off zero only when the last cell crosses. No exception, `window.__errs` empty.

### One more, outside the instrument's reach

`README.md` describes the guided path in prose, and the harness only walks `LESSONS`, so
nothing in the diff above could see it. It said the path "walks **eighteen** steps" —
already wrong before this slice, which found the count at nineteen — and its inventory
sentence ended at the two external shorts. Both corrected.

Worth naming the near-miss: the first search for this was `grep "19 steps\|nineteen"`,
which came back empty and was briefly taken as "the README is clean". It was searching
for the count the file *should* have had rather than for the claim, and the file was
stale in the other direction. The engine's own capability claims two screens up — the
`OCV(0)` forever paragraph and the reversal paragraph that replaced it — were checked
in the same pass and are correct.

## Versions

`WASM_API_MIN` moves **5 → 6**, and this is the bump `crates/sim-wasm/src/lib.rs`
predicted when it took `WASM_API_VERSION` to 6:

> `WASM_API_MIN` is deliberately **not** moved with it: a page that never touches the
> field has nothing to refuse.

The page now touches the field, so it has something to refuse. The failure mode without
the bump is the one the v4 entry describes: a v5 bundle serialises no `soc_deficit`, the
readout formatter multiplies `undefined` by 100, and the row prints `NaN pts` — a
displayed measurement of nothing, which is worse than the banner.

`SNAPSHOT_VERSION` and `sim_server::API_VERSION` do not move. The only Rust touched in
this slice is one doc comment (below); no behaviour, no type, no serialized shape.

## Also fixed: a label that could no longer be reached

The `clamp` readout row prints `refused` when `i_rejected_a` is negative and `invented`
when it is positive. `Telemetry::i_rejected_a`'s own doc now says **"Never positive"** —
the low clamp stopped reporting there when it stopped discarding charge — so the
`invented` branch names nothing any engine can produce. It is removed, and the row's
comment now says which end of the window it is about.

Two more places said the same thing and were found by grepping for the word rather than
for the code path: the current plot's `refused` trace, whose comment promised a positive
excursion "while an empty cell is sourcing charge it does not have", and `sim-wasm`'s v4
changelog entry, which introduced the field as "the charge a SOC clamp refused **or
invented**". The first is a comment on a live render path and is corrected; the second is
a historical entry and is marked as one rather than rewritten.

## Deferred, with a price

* ~~**Over-discharge is still free.**~~ **Done — `docs/plans/reversal-damage.md`.**
  Repayment is still exact in charge and no longer free in health. One caveat this bullet
  could not have known: no *shipped scenario* reaches the new damage, because the only one
  with aging enabled is a hot-storage run that never goes near empty. The engine change is
  complete and the client-side demonstration is its own deferred item there.
* **The two reversal constants per chemistry are still labelled placeholders.** Step 20
  quotes a −0.064 V floor and a 2 % collapse window; both are `floor_v` and `v_per_soc`
  read back, and the step says so rather than implying a measurement of a real cell.
* **Step 14's SPM floor is still a hole in a table**, and still described as one.
* **The pre-reversal 1.79 V figure was never right**, and correcting it here is
  incidental — the sentence containing it is being rewritten anyway.
