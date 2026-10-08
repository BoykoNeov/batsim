# Phase 9, slice D — the many-particle cell, taught

**Status: BUILT 2026-10-08. Phase 9 is complete.** Plan: `phase-9-lfp-ensemble.md` §D and exit
criterion 8. Harness, probe and raw output: `W:/temp/claude/phase9d/` (`probe/` is the page-shaped
measurement, `perturb.py` the table below, `walk.mjs` the browser walk).

## The owner's decisions, recorded

Asked and answered on 2026-10-08, before any lesson prose:

1. **Add a particle view.** Without one the plateau looks like any LFP curve and the step's
   central sentence — the particles fill in turn — is something the reader takes on trust. The
   cost was a field on the wire, a `WASM_API_VERSION` bump and a rebuild.
2. **Two cold seeds, as two files.** The page has no seed control, so the cold gap's being a
   draw is shown as a second pair of files that differ from the first in `seed` alone.
3. **A control arm.** The same experiment on the equivalent circuit, the same chemistry file,
   seed and road, so "the particles make the gap" is a subtraction.

## What was built

* **The engine half of the particle view.** `Pack::positive_particles(s, p)` — each positive
  particle's mean stoichiometry, `None` on every model but `SpmEnsemble` — through
  `CellModel::positive_particles` and `ensemble::positive_stoichiometry`. Not a `CellView`
  field: that view is `Copy`, and a fixed array would put `MAX_PARTICLES` numbers on the wire
  for every cell of every pack. A pure function of stored state, so `SNAPSHOT_VERSION` stays 25.
* **On the wire.** `sim_wasm::Cells` and the server's `/cells` response gain `particles`: one
  list per cell, or `null` for the whole pack when the model has one positive particle (so a
  100S10P circuit pack does not carry a thousand nulls a frame). `WASM_API_VERSION` 8 → 9 and
  the page's `WASM_API_MIN` with it — against a v8 bundle the field is `undefined` and the row
  would print "one per electrode" about a cell with twenty. `sim_server::API_VERSION` stays 2
  (an added key; its sixth parting from the wasm constant).
* **The page.** A `particles` readout row (`8 of 20 full`; "one per electrode" on the `Spm` and
  `Dfn`, "circuit — no particles" on a circuit), and the carrier diagram draws the positive
  electrode as one circle per particle, filled from the bottom by its lithium, in place of the
  carrier dots there. "Full" is past 0.5, the line `lfp_ensemble.rs` draws.
* **Eight scenario files**, every one `lfp_particles_charged.toml` with the fields its name
  says changed — `initial_soc` (`_discharged`), the cell model block deleted (`lfp_circuit_*`),
  `initial_temp_k` 263.15 (`_cold_`), `seed` 3 (`_seed3`) — which
  `the_particle_files_differ_only_where_their_headers_say` asserts over the whole `PackConfig`.
  Seed 9, not 0: it is the first of the four seeds the plan's criteria are pinned on, and seed 0
  is still switching after the cold rest. No BMS: this chemistry inhibits charge below 0 °C.
* **Guided-path steps 35–37** (`no-particle-is-halfway`, `which-way-it-arrived`,
  `a-throw-of-the-dice`), C/20 pulse for 36000 s then rest, `dt = 10 s`, 10 000×. Every numeral
  is tied: 22 claims (on the steps and their 6 arms), 3 derived subtractions, 9 ledger rules; all three steps are in
  the ledger whole and in `spelled` at 0.
* **`crates/sim-data/tests/lfp_particles_lesson.rs`**, for the sentences with no number in them:
  no particle between 0.1 and 0.7 of full at step 35's mark (11 at 0.80, 9 at 0.044); after the
  first hour of that charge never more than one particle between 0.3 and 0.7 at once; the
  circuit pair equal to 1e-9 V; the seed-3 cold charge arm within a microvolt of seed 9's; two
  particles switching during the seed-3 rest.
* The harness learned the `particles` row (`particles_full_at`, a mirrored formatter, three
  pins on the page's lines) and `HEADER_WORDS` thirty-five to thirty-seven.

## The numbers, measured on the page's shape

Out-of-tree probe (`probe/`, the page's `pulsePhase`, a zero-length probe at each mark):

| file | at | `terminal` (engine) | panel | `particles` |
| --- | --- | --- | --- | --- |
| `lfp_particles_charged` | 25200 s | 3.2702663 | 3.270 V | 11 of 20 |
| `lfp_particles_charged` | 43200 s | 3.2944996 | **3.294 V** | 8 of 20 |
| `lfp_particles_discharged` | 43200 s | 3.2748615 | 3.275 V | 4 of 20 |
| `lfp_circuit_charged` / `_discharged` | 43200 s | 3.26495000 / 3.26495000 | 3.265 V | — |
| `lfp_particles_cold_charged` | 43200 s | 3.2985837 | 3.299 V | 8 of 20 |
| `lfp_particles_cold_discharged` | 43200 s | 3.2711395 | 3.271 V | 5 of 20 |
| `lfp_particles_cold_charged_seed3` | 43200 s | 3.2985835 | 3.299 V | 8 of 20 |
| `lfp_particles_cold_discharged_seed3` | 43200 s | 3.2752685 | 3.275 V | 6 of 20 (4 at 36000 s) |

Gaps: 19.638 mV at 298 K, 27.444 and 23.316 mV at 263 K — `lfp_ensemble.rs`'s criteria 3 and 4
on these seeds. No solve went unconverged on any of the eight.

### Three things measured that the draft got wrong

1. **3.294, not 3.295.** The room-temperature charge arm rests 0.42 µV under the rounding edge,
   so the panel prints 3.294 V and the gap read off the two panels is 19 mV, not 20. The first
   headers said 3.295. The display check now holds the printed digit; a change to the cell that
   moved this voltage by half a microvolt would redden it, which is the point.
2. **"To every digit and beyond"** was written about the two cold charge arms. They are 0.13 µV
   apart: the radii differ, the count does not. The headers and the test say a microvolt.
3. **"One at a time" was true of one window, and the final review caught it.** After the first
   hour of the room-temperature charge never more than one particle is between 0.3 and 0.7 of
   full — but up to four are in 0.25–0.75 and seventeen inside the whole unstable region,
   because the full particles drift down together before each tips over. The first prose said
   "no particle starts to switch until the previous has finished", which the window made look
   true. Step 35 now says what was measured: the full ones give up a little lithium together,
   then tip over in turn, and no two are caught halfway across at once. On the discharge and in
   the cold two or three cross together even in 0.3–0.7, so only this run's prose says it.
4. **Step 36 first said the memory is made by the particles, without scope.** The file's one
   fitted constant (Ω, `[spm.positive.regular_solution]`) was fitted to Dreyer 2010's 20 mV
   rest gap, so the gap's *size* at room temperature was put into the file; its existence and
   direction are the particles'. The step now says so, and step 37 says the cold widening is
   the model's own. Same family as the gauge lesson's hand-picked `voltage_sigma_v`
   (`path-gauge-filter-steps.md`): a fitted setting under a headline is a claim the prose
   must scope.

### Two things the lesson could not write, and how they were written instead

* **"One at a time"** — the English-quantity ban reads "a time" as a quantity. "In turn".
* **The cold gaps over 20 seeds** (27.44 on 11, 23.32 on 4, 17.48 on 4, 13.35 on 1, slice C's
  note). The owner's choice was for the text to state them. It does not, yet: every numeral in
  a lesson must be tied, and no test holds these — only the four pinned seeds — so the prose
  says "a few more such values, the one this file shows the most common" and sends the reader
  to the cold file's header, which lists them. Putting them in the text costs a 20-seed test;
  that is the owner's call and is put to them.

The arm instructions first said "take the minus sign off"; `every_arm_is_instructed_by_its_own_step`
requires the current an arm types to be printed in its instruction, so each now names
`0.11517255`, tied by a ledger rule to its own arm. A test named `a_second_draw_…` was renamed:
the ban reads "a second" as a duration even inside a test name.

## Perturbations

Predictions written before running; each case edits one thing, runs
`cargo test -p sim-data --no-fail-fast` below normal priority, and restores the file from the
saved text. Each run executed 346 of `sim-data`'s tests (counted from the log, so a dropped
binary would show).

| case | change | predicted to redden | reddened |
| --- | --- | --- | --- |
| **A** | page's "full" threshold `> 0.5` → `> 0.9` | the mirrored-pin check alone | `mirrored_constants_still_match_the_page` alone — held |
| **B** | `CellModel::positive_particles` answers `None` on the ensemble | the three lesson tests; every `particles_full_at` claim | `every_claim_matches_the_engine` and all three tests in `lfp_particles_lesson.rs` — held |
| **C** | `lfp_particles_discharged` seed 9 → 3 | the pair test alone (seed 3 gives the same 19.64 mV at 298 K) | `the_particle_files_differ_only_where_their_headers_say` alone — held, and it is the **only** guard: at 298 K seed 3 rests on the same counts, so no claim moves |
| **D** | `lfp_particles_cold_discharged_seed3` seed 3 → 9 | the pair test, the claims, the cold-draw test | the pair test, `every_claim_matches_the_engine`, `another_draw_in_the_cold_moves_only_the_discharge_arm` — held |
| **E** | step 35's `11 of 20 full` → `12 of 20 full` in the prose | the literal check | `every_claim_appears_in_its_own_step` **and** `every_numeral_in_a_ledgered_step_is_accounted_for` — not alone: the stray 12 is a numeral nothing ties |
| **F** | control: an unclaimed sentence reworded | nothing | nothing (346 passed) — held |
| **G** | `particles = 20` → `19` in the charged file | the pair test, the ledger rule, the display claims, the lesson tests | the pair test, the ledger (the rule's `20` against the file's 19), `every_claim_matches_the_engine`, and the two room-temperature lesson tests — held; the cold one stays green because no cold file changed |

## Gates

* `cargo test --workspace --no-fail-fast` (below normal priority): **780 passed, 0 failed**, 92
  test binaries. `cargo clippy --workspace --all-targets -- -D warnings` and
  `cargo fmt --all --check` clean.
* `web/pkg` rebuilt from this tree; `node tools/wasm-parity/parity.mjs`: **156/156 bit-identical**
  (78 runs, 78 native-snapshot handoffs) — 16 of those runs are the eight new files, so the
  many-particle cell is now measured browser-against-native, not assumed.
* **Walked in a real page** (`walk.mjs`): `sim-server` on its own port, headless Chrome on its own
  profile and port, Start, Next to step 35, each mark read off `#readouts`. Step 35: `7.0h`,
  `3.270 V`, `-0.115 A`, `35.0 %`, `11 of 20 full`; step 36: `12.0h`, `3.294 V`, `0.000 A`,
  `50.0 %`, `8 of 20 full`; step 37: `3.299 V`, `-10.0 / -10.0 °C`, `8 of 20 full` — every
  claimed string. Each step reached its mark in under 4 s of wall clock at 10 000×. The diagram
  at step 35 shows eleven circles about four-fifths full and nine nearly empty. Chrome closed
  through CDP `Browser.close`, the server stopped by the PID recorded at launch.

## What this slice does not do

* **The arms are not walked in the browser** — the walk stops at the three marks. Every arm is a
  native run in `path_claims.rs`, and `tools/wasm-parity/parity.mjs` runs every shipped scenario,
  these eight included, through the browser build against the native one.
* **The server's `/cells` gains the key and no client reads it.**
* **The cold-gap distribution over many seeds is a slice-C measurement, not a test.**
* What Phase 9 leaves of H1 is in `phase-9-lfp-ensemble.md` §"What this phase does not close".
