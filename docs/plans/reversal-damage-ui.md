# The damage, shown to a reader — and four count claims that had already drifted

`docs/plans/reversal-damage.md` closed with this, under "Deferred, with a price":

> What remains deferred is narrower and is the ordinary kind of gap: the **guided path**
> has no step about it, so a reader who does not load that scenario by name still meets
> over-discharge as a debt that gets repaid. Step 20 is where it would go, and it is a
> client slice.

It is a client slice, and no Rust moved. But "step 20 is where it would go" was wrong in a
way that only survives until you try it: **step 20 runs on `cc_discharge_lfp.toml`, which
has `[pack.aging]` off.** A pack that cannot wear out has nowhere to put over-discharge
damage, so nothing that step could be told to watch would move. The damage needs the one
scenario built for it, and a scenario change is a *different step*.

So the path grows a twenty-first: **`what-it-cost`, "The part that does not come back"**,
on `over_discharge_damage_lfp.toml`. Steps 20 and 21 are now deliberately the same
experiment with one line of TOML between them — 20 shows the debt being repaid exactly, 21
shows what the trip down there cost — which is a better lesson than one step trying to
carry both.

## The step

```js
{
  id: "what-it-cost",
  scenario: "over_discharge_damage_lfp.toml",
  demand: { mode: "Current", value: 2 },
  ambient_c: 25, bms: null, dt: 0.5, speed_x: 50,
  until_s: 600, reload: true,
  watch: ["plot-soh", "readouts"],
}
```

Four of those fields are load-bearing rather than stylistic:

* **`reload: true`** — the pack starts at 5 % charge at `t = 0`, and the 600 s mark is
  below almost every earlier step's, so neither clause of `applyStep`'s inheritance rule
  would fire on its own.
* **`dt: 0.5`** — pinned for step 18's reason and for one more. `aging.rs` **carries** a
  partial sub-clock period rather than dropping it, so the trajectory depends on the
  *sequence* of step sizes a client feeds and not only on their sum. Step 18 actively
  instructs a reader to put that box to 5 and then 10.
* **`bms: null`** — the scenario ships no `[pack.bms]`, so the checkbox is disabled;
  `null` leaves it rather than fighting it.
* **`until_s: 600`** — the same mark the scenario file's own `description` quotes its
  numbers at, so a reader who loads the file by name from the picker and a reader who walks
  the path are told the same things.

`speed_x: 50` is slower than either neighbour. The whole run is ten minutes and the part
worth watching starts at three and a half; at step 20's 100× the collapse is over before a
reader has found the right row. (The slider is the base-10 log of the multiplier on a 0.02
grid, so the label reads **50.1×** — the same quantisation step 3's 200 already lives with.
Nothing in the prose quotes it.)

## The instrument

`M:\claud_projects\temp\reversal-numbers`, the harness `docs/plans/reversal-ui.md`
describes, with a `rev-damage` run added. Two arms, not one:

* the shipped chemistry, discharged at 2 A to the 600 s mark and then charged back at −2 A
  from exactly that state;
* **a control with `[reversal] fade_per_ah` zeroed in the parsed chemistry and nothing else
  touched.**

The control is the point. "The cell lost 4.84 % of its capacity" invites a reader to credit
all of it to the over-discharge, and calendar and cycle fade are running the whole time.
Subtracting one arm from the other turns that attribution from an inference into a
measurement. Both arms are identical to the last decimal up to the knee, which is what they
should be — nothing has gone past empty yet — and that identity is a check on the harness as
much as on the engine.

The harness also prints **what the readout row itself shows**, through the row's own
formatters, and the first instant each printed value changes. That is the difference between
a number that is true and a number a reader can reach; it caught two claims below.

## Measured

Discharge leg, 2 A from 5 % SOC, `dt = 0.5`:

| | shipped | control (`fade_per_ah = 0`) |
| --- | --- | --- |
| empties (`SOC_CLAMPED_LOW`) | 207.5 s, 1.9306 V | identical |
| `soh cap` at the knee | 99.98 % (99.975252) | identical |
| terminal through 0 V | 287.5 s | 288.0 s |
| `soh cap` at 250 s / 300 s | 99.45 % / 98.84 % | 99.97 % / 99.97 % |
| **`soh cap` at the 600 s mark** | **95.16 %** (95.156970) | **99.96 %** (99.956915) |
| **`soh res` at the mark** | **1.0726 ×** | 1.0006 × |
| deficit at the mark | 9.704 pts | 9.475 pts |
| charge delivered past empty (as billed) | 0.2182 A·h | 0.2182 A·h |

So of the **4.82** points of capacity lost after the knee, **4.80 are the reversal** and
0.018 are calendar and cycle fade. The health readout is *already* off 100.00 % at
`t = 10 s`, the first tick of the aging sub-clock, which is why the step quotes the knee
reading before it quotes anything else.

Two rows in that table are worth more than their line.

**The same amp-hours are a bigger deficit in the damaged arm** — 9.704 points against
9.475, from an identical 0.2182 A·h. The deficit is a *fraction* of the capacity the cell
has now, and the shipped arm's cell is 4.8 % smaller. This is the same effect
`reversal-damage.md` recorded on the repayment side, seen from the other end.

**The floor is not flat.** Step 20's reader is told the terminal stops at −0.0640 V and
stays there; the control arm reproduces exactly that, to four decimals, for 300 s. The
shipped arm does not — it reads −0.0648 V at 300 s and −0.0672 V at the mark, and the
`terminal` row prints −0.065 and −0.067. That 3.2 mV is `I · R0 · (soh_res − 1)`:
2 A × 0.022 Ω × 0.0726. It is checkable arithmetic in a lesson, and it is the only place on
the page where resistance growth shows up as a *voltage*.

### Which amp-hours — the two conventions differ in the fourth decimal

The naive integral, `∫I dt` over every step whose *post*-step deficit is nonzero, reads
**0.218333 A·h**. It over-counts, by exactly the pre-empty part of the step that crosses
zero: 2 A × 0.5 s is 0.000278 A·h and only some of it came out past the boundary.

What the engine bills is the deficit's *increase* valued against the capacity it is a
fraction of — `Δdeficit · eff_cap · soh_cap`, read **before** the step's aging tick, which
`reversal-damage.md` argues at length and pins with `reversal_ah_matches_current_integral`.
That reads **0.218179 A·h**, and it is the one quoted: **0.2182**.

This matters because the scenario file's own `description` — which the picker shows —
already carried this number, as **0.2181**. A reader who loads the file by name and then
walks the path sees both. Reconciled to 0.2182 in both places, and in
`reversal-damage.md`'s record of the same measurement.

Recharge leg at −2 A from the mark:

| | |
| --- | --- |
| deficit reaches zero | **+383.0 s**, 0.2128 A·h in against 0.2182 A·h out |
| `soc (true)` at that instant | 0.0040 % — the row prints **`0.0 %`** |
| `soc (true)` first prints `0.1 %` | +385.0 s |
| `soh cap` prints 95.15 % from | +60.0 s |
| `soh cap` prints 95.14 % from | +390.0 s |
| `soh cap` after 700 s of charging | 95.1366 % — **still falling** |

## Two numbers that were true and unreachable

Both were in the draft, both were caught by printing the readouts rather than the floats,
and both are the defect class `docs/plans/path-numbers.md` named: *right about a quantity
nobody can look at.*

* **"at t = 983.0 s on the clock."** `fmtTime` prints whole minutes above 120 s, so the
  clock reads `16m` at that instant and `10m` at the mark before it — six characters for
  383 seconds. Replaced with what actually marks the instant — `past empty` ceasing to
  print a number and reading `none` — and the step now says outright that two of its rows
  will not tell you when. (The first draft of the replacement said `17m`, because the
  browser walk *observed* 17m: the driver polled through the very sampling lag the step
  warns about two sentences later. `fmtTime(983)` is `16m`. A number read off an instrument
  you have just documented as lagging is not a measurement of the thing.)
* **"`soc (true)` leaves zero on that same step."** True of the engine, false of the panel:
  the cell is 0.004 % full when the debt clears, and the row prints `0.0 %` for another two
  seconds. Now stated as the thing it is — a row that is still saying zero after the zero
  has gone.

## And one caveat about an instrument, because the step asks a reader to read instants

`past empty` does not ride the telemetry frame the rows above it ride. It comes from
`refreshCells`, throttled to 250 ms of **wall** time, so at this step's speed it can be a
dozen seconds of *simulation* behind. That is not hypothetical and not calculated: the
browser walk below caught the row reading **9.438 pts** at the instant the run stopped at
its mark and **9.704 pts** on the next sample after the pause. (A quarter-second is the
*throttle*, so it is the bound on how long the row can stay wrong once nothing is moving —
not a measured interval; the driver's own gap was longer.) Both numbers are in the step's prose,
because the useful lesson is that the lag exists and closes on a pause, not that it does not
exist. `soh cap` and `soh res` have no such lag.

This applies to step 20's mid-run deficit figures too, which are engine-true and were
measured that way. They are not falsified — the step never promises the row will show them
at that instant — so nothing there is being changed on the strength of it.

## Verified on the page, not only in the harness

`M:\claud_projects\temp\page_walk21.py`: headless Chrome over CDP against
`cargo run -p sim-server`, walking the path to step 21 and then **walking the instructions
the prose hands out** — the memory rule that a lesson step which names controls is a control
path and must be pressed. Set the demand box to −2, dispatch its `change`, press Run, watch
`past empty` go to `none`, pause, press Step 1.

Read back at the mark, from the real panel: `terminal −0.067 V`, `soc (true) 0.0 %`,
`soh cap 95.16 %`, `soh res 1.0726 ×`, `past empty 9.704 pts`, flag `SOC_CLAMPED_LOW`, and
the state-of-health plot showing both traces flat at 100 until the knee and then leaving it
in opposite directions. After the recharge: `past empty none`, `soh cap 95.14 %`. No
uncaught exception and nothing on `console.error`.

The step also applies as written — scenario, `dt` 0.5, `Current` 2, ambient 25 °C, the BMS
box disabled and left alone, the in-page engine rather than the socket, and `readouts` and
`plot-soh` outlined.

One driver note worth keeping: `--window-size` does not reach the renderer in this headless
build, so every screenshot was of the page's narrow single-column fallback and the plots
were invisible. `Emulation.setDeviceMetricsOverride` is what actually sets the viewport.

## The four count claims, hunted by claim rather than by number

Appending a step falsifies every sentence that says how many there are, and one that says
which one is last. Searching for `20` would have found one of these.

* **`web/index.html`** — `Start — 20 steps`, the pre-script label. → 21. (`app.js` derives
  the live label from `LESSONS.length`, so only this one is hand-written.)
* **`README.md`** — "It walks twenty steps". → twenty-one, and the inventory sentence gains
  the new step rather than merely being recounted.
* **Step 1's "what to watch"** — "That is the subject of the last step in this path." → step
  20, plus a pointer at 21.
* **Step 2's** — "That fall is the last step of this path." → step 20.
* **Step 14's** — "The last step of this path is about that choice." → step 20, plus a
  clause on what 21 adds.

And one that was **already stale before this slice**, found while checking whether the new
step's `reload`/`dt` fields belonged in it. `README.md` said reloads are asked for by
"steps 12 to 16" and that "those five also pin the timestep". Neither was true: 17 also
reloads and pins, 18 and 19 pin without reloading, and 20 does both. Corrected to what the
records actually say — 12 to 17 and the last two reload, and **every** step from 12 on pins
the timestep — with the reason step 18 is the exception that makes the pins necessary.

## Versions

**Nothing moves, and that is the checkable claim.** `WASM_API_MIN` is already 6; the step
reads `soc_deficit`, `soh_capacity` and `soh_resistance`, and all three were on the wire and
already rendered by the readout row before this slice. `WASM_API_VERSION`,
`sim_server::API_VERSION` and `sim_core::SNAPSHOT_VERSION` are untouched because no Rust is.

`web/pkg` **was** rebuilt, and the version constants are exactly why that has to be said out
loud: a bundle built before v15 is still api 6, so the page's version check cannot see it,
and `serde` ignores unknown TOML keys — it would read the new chemistry file, silently drop
`fade_per_ah`, and show a reader a health readout that never moves beside prose promising
4.84 %. The check that matters here is not a constant; it is the rebuild.

## Noticed, not fixed: `CLAUDE.md` and the code disagree about resistance growth

Working out the floor's 3.2 mV sag needed the arithmetic to close, and it only closes if
`soh_resistance` multiplies **`R0` alone**. It does: `ecm.rs`'s RC update passes
`pair.r_ohms` unscaled, `CellView::soh_resistance`'s own doc says "effective `R0` = nominal
× `r0_factor` × this", and the measured RC overpotential is 0.020000 V — `2 A × 0.010 Ω`
exactly — in both arms while their terminals differ.

`CLAUDE.md`'s physics spec says something else: *"effective R0 and RC resistances = nominal
× `soh_resistance`"*. One of the two is wrong and it is pre-existing, unrelated to this
slice, and not a client-slice decision — changing the code would move every golden, and
changing the spec is the owner's call. Recorded here; nothing edited.

## Deferred, with a price

* **The three `[reversal]` constants are still labelled placeholders**, `fade_per_ah`
  included. Step 21 quotes 4.80 points of capacity loss as a *measurement of this model*,
  which it is, and does not claim it is a measurement of a cell. A fit would change the
  number and not the lesson.
* **The resistance coupling is still shared**, so the 1.0726 × the step makes a reader look
  at is the generic "each point of capacity costs 1.5 points of resistance", not a fitted
  over-discharge number. The step's checkable 3.2 mV is checkable arithmetic about *this
  engine* either way.
* **No porous-electrode arm.** The `Spm` and `Dfn` cells never clamp, carry no deficit, and
  cannot reach this mechanism at all — so the guided path's over-discharge pair is
  circuit-only, and step 14's note about the particle's floor is still the only thing said
  about what those models do down there.
* **Nothing here is asserted by a test.** The path's numbers never have been; that is what
  the harness exists for, and it is out of tree.
