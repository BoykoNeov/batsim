# The fifth ledgered step, and the two sentences it rewrote

`docs/plans/path-ledger-scenario-arm.md` built the ledger — a scan over a step's *whole*
prose, requiring every numeral in it to be tied to something — and
`path-ledger-fourth-step.md` took it as far as `protection-on`, four steps and twenty-two
numerals, on arms that read the scenario, the chemistry, a control on the lesson block and
the claims already on the sentence. This slice takes it to `circuit-repeats-itself`, the
pulse-train step, which is the first ledgered step whose numbers are **mostly
measurements**.

**What landed.** The ledger goes from four steps and twenty-two numerals to **five and
forty-two**; the file goes from 169 claims to **175**. Four new ties exist (a part number, a
cross-reference to another step, a table node, and two new lesson controls), one new frame
exists on an old arm, and **two sentences of the lesson were rewritten** — which is the part
of this slice worth reading.

## The sizing was wrong before it was measured

The previous plan ranked this step as the cheapest next target at "one unplaceable numeral",
using its own *generous* classifier — the one it says twice is for ranking and must then be
thrown away. Taken strictly, the step prints twenty-two numerals of which **fourteen** were
unaccounted, and two of those had no honest arm available at any price.

The correction was mechanical, not another estimate: the ledger's panic was temporarily
swapped for a collector, both candidate steps were opted in, and one run printed every
numeral with what did or did not account for it. That is the instrument this file should
have been ranked with in the first place, and it is three minutes of work.

**The lesson generalises past this step.** A classifier that accepts "this number appears
somewhere in that file" understates the bill by whatever fraction of a step's numbers are
*names, ordinals and symbols* rather than quantities — and prose is full of those. Rank with
it if you like; size with the scanner.

## Two numbers that were not numbers, and what was done about them

### `I·R0` — no arm exists, and none should

The prose wrote the instant voltage drop as ``I·R0``. The `0` in it is a subscript that lost
its typography: it names no field, measures nothing, and is not a quantity. Three ways out
were put to the owner — write it as `R₀` (U+2080 is not an ASCII digit, so the scanner stops
seeing it), reword to drop the symbol, or build a tie that matches a symbol against the
`[r0]` section name in the chemistry file. **Reworded**, which is the choice that leaves
nothing clever behind:

> the jump is the current times the cell's series resistance, and is over immediately

The symbol survives in steps 2 and 21, both unledgered. Whoever ledgers those meets this
question again, and the answer is written down here so it does not have to be re-argued.

A second thing fell out of the reword. The decomposition sentence used to be claimed as
*two* literals, split around `I·R0` precisely because the `0` sat between them and check 6
would have demanded an accounting for it. With the symbol gone the split has no reason, so
the two halves are one sentence again with four claims on it — sag, jump, rebound, lost —
and the duplicate rebound claim that paid for the split is now the ordinary case of one
measurement stated in two different sentences.

### "9 mV deeper" — one number that was three

The prose said teeth 4 and 5 are `9 mV` deeper than teeth 1 to 3. Measured, the five sags
are **212.8146, 212.6397, 212.4680, 221.4615 and 221.2908 mV**, and the sentence's claim has
no single value:

| reading of "9 mV deeper" | value | inside a tolerance spelled by `9`? |
| --- | --- | --- |
| shallowest late tooth − deepest early tooth | 8.4761 mV | **no** (0.524 out, against ±0.5) |
| mean of teeth 4–5 − mean of teeth 1–3 | 8.7354 mV | yes |
| the step itself, tooth 4 − tooth 3 | 8.9935 mV | yes |

Two of the three round to 9 and the weakest one does not. **Choosing among them after seeing
which reproduces the digit is the declaration hazard `path-ledger-fourth-step.md` names**, in
the same shape as its Arrhenius ratio: the formula that gives you the number you wanted is
not evidence about the number. So the difference was deleted in favour of the measurements,
which is the one option that adds information instead of picking a definition:

> you will find the teeth sit at two levels: 212.8, 212.6 and 212.5 mV for the first three,
> then 221.5 and 221.3 for the last two

Five claims, one per tooth, each pinned at the precision the prose prints. The four tooth
ordinals (`4`, `5`, `1`, `3`) went with the sentence, which is why this step needs no
tooth-numbering arm.

**The lesson's *explanation* was right and is now checked from both ends.** Tooth 3 ends at
soc 0.850002 and tooth 4 is the first that spends its whole length below the `[ocv]` table's
node at 0.85 — so the step really is the table node passing under the pulse, exactly where
the prose says. The node itself is now tied to the chemistry file (`Tie::Member`), and the
five depths to the engine.

## The arms this cost

| numeral | arm | what decides it |
| --- | --- | --- |
| `60`, `600` | setting | `Control::PulseOn` / `PulseOff` — the page's own demand program |
| `50` | name | the digits after `M` in the chemistry's `meta.name` |
| `2` | ordinal | the position of the lesson `same-discharge-other-chemistry` |
| `90` | scenario | `pack.initial_soc` |
| `85` | member | a node of the chemistry's `ocv.soc` |
| `300` | read at | a `pulse_rebound_arrived` claim's instant, **since the current stopped** |
| the other fourteen | claimed | claims on the sentences that print them |

Four notes on those, each a decision that could have gone the lazy way:

* **`Tie::Name` carries a prefix, and that is the whole of its honesty.** `meta.name` is
  `"LG M50 21700 (NMC811/graphite)"` — three digit runs. A tie that asked whether `50`
  appears *anywhere* in it would be the generous match the vocabulary table exists to refuse,
  and would still be green the day the sentence said `M811`. The tie collects only the runs
  that follow `M`.

* **`Tie::Member` is the one existential tie, and it is existential because the sentence
  is.** "The table's node at 85 % charge" claims that 0.85 is *a* node, not that it is the
  twenty-first — a fact about table layout no reader is shown and a re-fit would change
  without making the sentence wrong. `Tie::Scenario`'s wildcard is strict for the opposite
  reason. **The price, stated rather than discovered:** on a table with thirty-four nodes, a
  mistyped node has a real chance of being some *other* node and passing. Perturbing `85` to
  `86` reddens; perturbing it to `90` would not.

* **`Tie::Ordinal` names the step and derives the number.** "From step 2's aside" is one
  insertion away from pointing at the wrong lesson, silently. The rule declares
  `same-discharge-other-chemistry` and the array says where it is.

* **The rest-relative reading is fenced to one quantity.** A rest is a leg with an origin of
  its own, exactly as a continuation is, so `Accounted::ReadAt` now also accepts a claim's
  instant counted from the moment its tooth's current went off — 360 − 60 = 300. Given to
  `pulse_rebound_arrived` alone: every other pulse quantity is read *at* a leg boundary,
  where this reading would return the leg length itself, which is a number the prose writes
  as the demand program and which has a vocabulary rule of its own. Two readings of one
  number is what the whole taxonomy is arranged against, and this is where it would have come
  back.

## Reddening

Fourteen perturbations, fourteen reds, each verified on **its own assertion** — the failing
test named and its message matched, with the child launched directly under
`BELOW_NORMAL_PRIORITY_CLASS` and its real exit code read (never through `start /wait`,
which is exit-code-blind here and has now lied twice).

| perturbation | reddens |
| --- | --- |
| prose says the current runs for `61 s` | the `PulseOn` tie |
| the lesson block's `on_s` moves to 61 | the same tie, from the block's side |
| prose says the rest runs for `601` | the `PulseOff` tie |
| prose calls the cell an `M51` | the name tie |
| the chemistry file renames the part | the same tie, from the file's side |
| prose points at `step 3's aside` | the ordinal tie |
| prose says the pack starts at `91 %` | the scenario tie |
| the scenario's `initial_soc` moves to 0.91 | the same tie, from the file's side |
| prose names a `node at 86 %` | the member tie |
| **prose and claim say `301 s` together** | check 6 — the rest-relative reading |
| the rest-relative reading is removed from the code | check 6, on the same `300` |
| **prose and claim deepen tooth 4 to `221.6` together** | check 5 — the prose against the engine |
| the tooth-5 claim is deleted whole | the ledger, on `221.3` unaccounted |
| the closing rebound claim is deleted whole | the ledger, on `74.8` unaccounted |

The two in bold are the ones worth naming. Changing the prose *alone* reddens check 1 —
the claim's quoted text is no longer in the page — which is a real failure and the wrong
one: it proves the literal check works and says nothing about the accounting. Drifting the
prose and the claim **together** is the defect checks 5 and 6 exist for, and it is the only
perturbation that reaches them.

## Deferred, with a price

* **The general `Derived` arm is now the last one missing** from
  `docs/plans/path-prose-ledger.md`'s taxonomy — a figure computed from its siblings in the
  same sentence. Ordinals, part numbers, table nodes and controls all have one. Step 8's
  `2.7×` (2.84 / 1.06) and step 17's `12 V` (six 2 V cells, where the *six* is spelled in
  letters) are the two sentences waiting on it, and the second one needs the word scanner as
  well.

* **Word numerals are still invisible, and this step just added three.** "Two levels", "the
  first three", "the last two" are quantities a reader leans on and no instrument here sees
  them. A green ledger says a step's *digits* are tied to something; it does not say the step
  is checked. Unchanged from the fourth-step slice, but the gap is now slightly wider by this
  slice's own doing, which is worth being explicit about rather than quiet.

* **`slow-and-patient` is the cheapest remaining target**, measured by the same scanner in
  the same run: sixteen numerals, twelve unaccounted, and nothing blocked. Five are nearly
  free (`7.2` is `cell.capacity_ah` three times over, `1.75` is `cell.v_min`, `0.36` is the
  demand box), two need only a claim (`19.3h` off the clock row, and a second `3.3 %`), and
  the ordinal arm this slice built covers its "the LFP cell of step 1". What it needs beyond
  that: a quotient tie for `C/20`, a table-span tie for its `180 mV`, and the `Derived` arm
  for `12 V`.

* **The one number this slice measured and did not use.** Tooth 4 is 8.9935 mV deeper than
  tooth 3 — the actual size of the step at the node, and a perfectly good figure for a
  sentence that wanted one. It is written here and not in the prose, because the prose now
  prints the five depths and a reader can do the subtraction themselves against numbers that
  are each pinned.
