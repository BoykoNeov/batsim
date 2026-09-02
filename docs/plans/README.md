# `docs/plans/` — an index

Every slice of work in this repository leaves one of these behind: a design note written
*before* the run, with its predictions registered, and a results section written after,
with the predictions scored, a perturbation table showing which tests catch which break,
and a list of what was deliberately not built. They are the project's memory. The habit
that produced them is described in `CLAUDE.md`; where the project goes next is
`docs/ROADMAP.md`, which reads across these to list every hurdle they record as open.

How to read one: the results section is the part that matters, and the honest ones say
where a prediction failed. A **"Still open"** or **"Deliberately not done"** section at the
foot is the contract for the next slice. A struck-through item (`~~like this~~`) or a
**"Closed — see …"** line means a later document took it up; follow the pointer rather
than the original.

Grouped by what they are about. Within a group the order is chronological where it
matters (the phases) and alphabetical where it does not.


## Phase records

One per phase, the slice notes under it. Each records what was measured, what was built, and what was deliberately not.

| file | what it records |
| --- | --- |
| [`phase-2-thermal-bms.md`](phase-2-thermal-bms.md) | Phase 2 — thermal + BMS |
| [`phase-3-aging-faults.md`](phase-3-aging-faults.md) | Phase 3 — aging + faults |
| [`phase-4-server-wasm.md`](phase-4-server-wasm.md) | Phase 4 — headless server + browser demo |
| [`phase-5-godot.md`](phase-5-godot.md) | Phase 5 — Godot adapter |
| [`phase-6-porous-electrodes.md`](phase-6-porous-electrodes.md) | Phase 6 — porous electrodes (`Spm`) |
| [`phase-7-dfn.md`](phase-7-dfn.md) | Phase 7 — the electrolyte (`Dfn`) |
| [`phase-8-chemistries.md`](phase-8-chemistries.md) | Phase 8 — new chemistries |
| [`phase-8-slice-a-lto.md`](phase-8-slice-a-lto.md) | Phase 8 slice A — one chemistry, and whether principle 10 survives it |
| [`phase-8-slice-b-lto-client.md`](phase-8-slice-b-lto-client.md) | Phase 8 slice B — wiring the LTO cell into the client, and teaching it |
| [`phase-8-slice-c-hysteresis.md`](phase-8-slice-c-hysteresis.md) | Phase 8, slice C — the hysteresis state, the OCV temperature correction, and NiMH |
| [`phase-8-slice-c-spike.md`](phase-8-slice-c-spike.md) | Phase 8, slice C — the spike, and what it decided |
| [`phase-8-slice-d-nimh-client.md`](phase-8-slice-d-nimh-client.md) | Phase 8 slice D — teaching the nickel cell, and closing the phase |

## Engine physics slices

Changes to what `sim-core` computes. Every one names the snapshot version it cost, if any.

| file | what it records |
| --- | --- |
| [`charge-acceptance.md`](charge-acceptance.md) | Charge acceptance: the third mechanism, and what a dome costs the signal |
| [`dfn-aging-gap.md`](dfn-aging-gap.md) | The DFN's aging gap: a rule implemented for three commits and verified by none |
| [`diffusion-overpotential.md`](diffusion-overpotential.md) | A diffusion overpotential for lead-acid — can the ECM carry Peukert at all? |
| [`energy-hole.md`](energy-hole.md) | The energy hole: charge that vanishes at the clamp, and charge that appears |
| [`hysteresis-width-over-soc.md`](hysteresis-width-over-soc.md) | A hysteresis loop that is not the same width everywhere |
| [`low-clamp-reversal.md`](low-clamp-reversal.md) | The low clamp closed: a reversal branch below empty |
| [`low-clamp-solve-side.md`](low-clamp-solve-side.md) | The low clamp: a solve-side fix that was priced, measured, and does not work |
| [`plating-absence.md`](plating-absence.md) | A cell that does not plate can say so |
| [`rc-resistance-growth.md`](rc-resistance-growth.md) | Aging grows the RC resistances too — the spec was right and the code was not |
| [`reversal-damage.md`](reversal-damage.md) | Over-discharge that leaves a mark |
| [`surface-vs-bulk.md`](surface-vs-bulk.md) | Surface vs bulk: the gradient an equivalent circuit cannot have |

## Solver, demands and protection

The pack solve, the demand window, and the BMS comparators.

| file | what it records |
| --- | --- |
| [`balancing-chatter.md`](balancing-chatter.md) | Balancing chatter: the last bandless comparator, and a band sized by a different rule |
| [`cc-cv.md`](cc-cv.md) | CC-CV: the other half of the demand story, and the leg LFP does not have |
| [`operating-point-window.md`](operating-point-window.md) | The client naming a current did not know where it was going either |
| [`power-operating-point.md`](power-operating-point.md) | A power demand never says where it landed |
| [`protection-chatter.md`](protection-chatter.md) | Protection chatter: a comparator that oscillated, and the band that is not the one you would size |
| [`protection-escalation.md`](protection-escalation.md) | Protection escalation: the clamp that cannot reach a short, and the contactor that can |
| [`voltage-target-blowup.md`](voltage-target-blowup.md) | The solver blowing up on a voltage target outside a cell's range |

## Chemistry and parameter files

Slices that were meant to be data-only, and what they found about principle 10.

| file | what it records |
| --- | --- |
| [`lead-acid-data-only.md`](lead-acid-data-only.md) | Lead-acid as data alone — how far does it get? |
| [`sodium-ion-chemistry.md`](sodium-ion-chemistry.md) | A seventh chemistry — sodium-ion, fitted from published measurements |

## Scenarios and the browser client

What a reader can reach, and how the page shows it.

| file | what it records |
| --- | --- |
| [`dfn-scenario.md`](dfn-scenario.md) | The DFN scenario: a rate at which the particle model stops knowing the cell is dying |
| [`lead-acid-client.md`](lead-acid-client.md) | The lead-acid client slice: a cell that is not empty, and will not give you the rest |
| [`reversal-damage-ui.md`](reversal-damage-ui.md) | The damage, shown to a reader — and four count claims that had already drifted |
| [`reversal-ui.md`](reversal-ui.md) | Over-discharge, made visible — and four lessons that still described the old engine |
| [`scenario-catalog.md`](scenario-catalog.md) | The scenario catalogue, and the two scenarios nobody could load |
| [`spm-scenario.md`](spm-scenario.md) | The SPM scenario: a pulse train, and the two non-linearities that point opposite ways |
| [`ui-bms-view.md`](ui-bms-view.md) | UI / pedagogy — truth beside belief |
| [`ui-explanatory-path.md`](ui-explanatory-path.md) | UI / pedagogy — the explanatory path |
| [`ui-pedagogy.md`](ui-pedagogy.md) | UI / pedagogy — making the engine's own capabilities visible |

## Performance

Measurements of `Pack::step`, the instrument that takes them, and the rule for trusting one.

| file | what it records |
| --- | --- |
| [`cell-size.md`](cell-size.md) | `Cell` is 264 bytes and the recorded hypothesis is stale |
| [`pack-step-allocations.md`](pack-step-allocations.md) | The per-step allocations, removed — and counted rather than timed |
| [`pack-step-perf.md`](pack-step-perf.md) | `Pack::step` performance — four items landed; budget now marginal |

## Guided-path verification (`path-*`)

One continuous arc of bookkeeping over the lesson prose: claims, arms, the ledger that ties every numeral in a step to a measurement, and the digits rule. Internal to the client; the few engine findings in it are listed in `docs/ROADMAP.md`.

| file | what it records |
| --- | --- |
| [`path-accounting.md`](path-accounting.md) | The numbers in a claimed sentence that no claim was about |
| [`path-ambient-arm.md`](path-ambient-arm.md) | The ambient slider, and an equality asserted by two pins |
| [`path-arms.md`](path-arms.md) | The buttons the reader is told to press |
| [`path-article-shape.md`](path-article-shape.md) | The number English spells without a numeral |
| [`path-buttons.md`](path-buttons.md) | Step 18's two-button repair, measured |
| [`path-charge-legs.md`](path-charge-legs.md) | The charge legs: the half of two steps nothing could measure |
| [`path-claims.md`](path-claims.md) | The guided path's numbers, asserted by a test |
| [`path-derived-arm.md`](path-derived-arm.md) | The last accounting arm, and the deadlock it was half of |
| [`path-digits-rule.md`](path-digits-rule.md) | The digits rule |
| [`path-display.md`](path-display.md) | The panel's own numbers: closing the formatter gap |
| [`path-estimator-gap.md`](path-estimator-gap.md) | The estimator gap, measured — and the first number this path spells in letters |
| [`path-instant-tagged-readings.md`](path-instant-tagged-readings.md) | Eight voltages under one name, and the step that could not quote any of them |
| [`path-ledger-bare-curve.md`](path-ledger-bare-curve.md) | The first step, ledgered — and a note that had outlived its reason |
| [`path-ledger-dfn-step.md`](path-ledger-dfn-step.md) | The dense DFN step, scanned whole, and the five numbers that left the page |
| [`path-ledger-fifth-step.md`](path-ledger-fifth-step.md) | The fifth ledgered step, and the two sentences it rewrote |
| [`path-ledger-fourth-step.md`](path-ledger-fourth-step.md) | The fourth ledgered step, and the four arms it cost |
| [`path-ledger-idle-step.md`](path-ledger-idle-step.md) | The step where nothing happens, ledgered — and a number that was true of a run nobody makes |
| [`path-ledger-last-two-steps.md`](path-ledger-last-two-steps.md) | The last two lead-acid steps, scanned whole |
| [`path-ledger-leg-that-is-not-there.md`](path-ledger-leg-that-is-not-there.md) | The charge with no second leg, ledgered — and the third of the drop nobody had named |
| [`path-ledger-one-step-that-got-through.md`](path-ledger-one-step-that-got-through.md) | The step that is about its own step length, ledgered |
| [`path-ledger-particle-step.md`](path-ledger-particle-step.md) | The particle step, scanned whole, and two fences that were refusing the wrong thing |
| [`path-ledger-past-empty.md`](path-ledger-past-empty.md) | The step past empty, ledgered — and a hole three documents kept open that was never there |
| [`path-ledger-protection-off.md`](path-ledger-protection-off.md) | The step with nothing watching, ledgered — and a note that was wrong about its own subject |
| [`path-ledger-scenario-arm.md`](path-ledger-scenario-arm.md) | The ledger, built for the three steps that needed no measurement |
| [`path-ledger-sixth-step.md`](path-ledger-sixth-step.md) | The sixth ledgered step, and the last arm of the taxonomy |
| [`path-ledger-spm-step.md`](path-ledger-spm-step.md) | The half of the pair that looks fine, ledgered — and the neighbour that made it cheap |
| [`path-ledger-the-gradient.md`](path-ledger-the-gradient.md) | The last step, ledgered — and a diffusion time nobody had divided |
| [`path-ledger-third-cell-step.md`](path-ledger-third-cell-step.md) | The second step, ledgered |
| [`path-ledger-three-times.md`](path-ledger-three-times.md) | The step that quotes its neighbours — and a ratio two rounded figures got wrong |
| [`path-ledger-two-legs-step.md`](path-ledger-two-legs-step.md) | The charge, ledgered — and a sentence that counted the path wrong |
| [`path-ledger-weaker-short.md`](path-ledger-weaker-short.md) | The weaker short, ledgered — and three numbers that had to move |
| [`path-ledger-what-it-cost.md`](path-ledger-what-it-cost.md) | The step with a control arm, ledgered |
| [`path-ledger-what-protection-costs.md`](path-ledger-what-protection-costs.md) | The eleventh lesson, ledgered |
| [`path-numbers.md`](path-numbers.md) | The guided path's numbers, measured |
| [`path-probe-row.md`](path-probe-row.md) | The zero-length probe row |
| [`path-prose-ledger.md`](path-prose-ledger.md) | The fourteen steps nothing was checking, measured |
| [`path-prose-value-tie.md`](path-prose-value-tie.md) | The sentence's number and the engine's, joined |
| [`path-self-counts.md`](path-self-counts.md) | The file's account of itself |
| [`path-self-description-sweep.md`](path-self-description-sweep.md) | Four counts about these files were wrong, and all four were spelled in letters |
| [`path-sensor-quantity.md`](path-sensor-quantity.md) | The first claim that reads a sensor |
| [`path-setting-arm.md`](path-setting-arm.md) | A step length is a number too, and the numbers nobody was guarding |
| [`path-third-cell.md`](path-third-cell.md) | The third cell, reachable |
| [`path-tolerance-rule.md`](path-tolerance-rule.md) | The tolerance rule, enforced instead of written down |
| [`path-twin-arm.md`](path-twin-arm.md) | The arm that walks next door, and the three numbers it brought back |
| [`path-uniqueness-rule.md`](path-uniqueness-rule.md) | The uniqueness rule |
| [`path-untouched-steps.md`](path-untouched-steps.md) | The five steps nothing was about |
| [`path-wedge.md`](path-wedge.md) | The guided path's last step, and a freeze nobody has attributed |
| [`path-wider-loop-step.md`](path-wider-loop-step.md) | The lesson for a loop that is not one width |
| [`path-word-batch-five.md`](path-word-batch-five.md) | Word batch five — the millivolt, and the amp that was never about the word |
| [`path-word-batch-four.md`](path-word-batch-four.md) | The unit that was left out on purpose, and the two lessons it was holding |
| [`path-word-batch-three.md`](path-word-batch-three.md) | The third batch of word-scanned steps |
| [`path-word-batch-two.md`](path-word-batch-two.md) | The second batch of word-scanned steps |
| [`path-word-numerals.md`](path-word-numerals.md) | Word numerals in the ledger |
| [`path-word-still-in-there.md`](path-word-still-in-there.md) | The step that is nothing but hours |

## Adding one

Name it for the thing, not the phase (`protection-chatter.md`, not `phase-2-fix-3.md`),
unless it *is* a phase or a slice of one. Register predictions before the engine runs and
never edit them afterwards — score them. Record the perturbation table by naming which
tests reddened, not just that the exit code moved. End with what is still open, and add
the file to the table above.

