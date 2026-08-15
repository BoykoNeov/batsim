# The sixth ledgered step, and the last arm of the taxonomy

`docs/plans/path-prose-ledger.md` laid out six ways a number in a ledgered step's prose can
be tied to something, and one of the six had never been built:

> **The `Derived` arm is the one with no precedent.** Every other arm checks a token
> against a file; this one checks a token against other tokens in the same sentence … The
> cheap version — a list of declared identities — is a declaration, which is the thing the
> design refuses.

It also named the sentence that would need it, and the step it sits in:

> **`Tie::Derived` is still missing**, and the step that needs it — `slow-and-patient`'s
> "six of these in series is the 12 V battery" — is not ledgered yet.

This slice ledgers that step. It is guided-path step 22, the first lead-acid lesson, and
closing it took four new arms rather than one: the derivation the plan named, plus a
quotient and a table span, plus one the plan had not foreseen at all.

## The step, counted

Sixteen numerals in its whole prose. Four already had claims (`1.750 V`, `3.3 %`,
`6.9620 A·h`, `0.07 W`), which is what the file's check 6 reaches. The other twelve were
tied to nothing, and every one of them is a *constant* — which is what made this step the
cheap-looking one and is also what hid the work: not one of the twelve is a field a rule
could read straight off a file.

| number | tied to |
| --- | --- |
| `2` V | the cell's own **name** — `meta.name` is "Generic AGM lead-acid 2 V cell" |
| `12` V | **`Tie::Derived`** — six times the `2` this same sentence prints |
| `7.2` Ah, three times | the chemistry's `cell.capacity_ah`, through three phrases |
| `0.36` A | the demand box, `Control::DemandValue` |
| `20` in `C/20` | **`Tie::Ratio`** — the capacity over the current in that box |
| `1.75` V | the chemistry's `cell.v_min` |
| `180` mV | **`Tie::Span`** — the `[ocv]` table's largest value minus its smallest |
| `1` in "step 1" | `Tie::Ordinal("bare-curve")` |
| `19.3` h | **`Tie::Clock`** — what the `sim time` row prints at the step's mark |
| the second `3.3 %` | a **claim**, because the `claimed` arm is positional |

### The four new arms

**`Tie::Derived`** is the taxonomy's sixth and last. Its operands are declared and its value
never is: one is a numeral the same sentence prints (`Operand::Sibling`), one is a numeral
the sentence spells in letters (`Operand::Word`). Three fences make it a tie rather than a
declared identity — an operand must sit in the same sentence, must not be the number being
derived, and **must itself be accounted for by an arm that is not a derivation**. A
derivation resting on derivations has no floor.

**`Tie::Ratio`** divides one tie by another. `C/20` is the hours the rating is quoted over:
`cell.capacity_ah` over the demand box. Change the box to C/5 and the sentence is wrong
about the rate it names, which is exactly what a reader would be misled by.

**`Tie::Span`** is about a whole table rather than a node of it, which is what "lead-acid
spans only 180 mV end to end" says. Fenced to paths reaching two values or more: the span of
one value is zero, and a sentence claiming a chemistry spans nothing would otherwise be
accounted by an emptied table — a fail-toward-green on the restructuring the arm exists to
notice.

**`Tie::Clock`** was not foreseen by the plan. `19.3h` is not the mark (69620.5 s) and is in
no file at all: it is what one row prints when the run stops there. The tie is to that row's
formatter — the same `fmt_time` mirror the display check runs on, and the same reasoning
`Accounted::Shown` already gives for handing a claim the clock at the mark: `sim time` is the
only row that is a function of time alone, so it is the only one this scan can render
without an engine.

Three of the four are **computed**, so they compare at the precision the prose commits to,
through the page's own rounding rule — the reason `Tie::Product` already did. It is not
pedantry: 7.2 / 0.36 is 20.000000000000004 and 2.130 − 1.950 is 0.17999999999999994.

### `six` is read, and it is not a word scanner

`Operand::Word` resolves through `WORD_NUMERALS`, the table `spells` already reads, so a word
means one number in this file rather than two — and the check that every entry is used now
accepts either reader. **Nothing else changed.** The scan still finds digits, so "all but
three points" in this step's own expect is as invisible as it was, and so is "twenty hours"
two paragraphs above it. A green ledger is still a statement about a step's digits, and this
slice makes the sixth ledgered step the third to state a measurement in letters — five
across the path now, four of them checked by nothing.

### The one number that needed a claim

`3.3 %` is printed twice: once where the panel is quoted, once in the sentence that says what
*kind* of number it is ("a stated model error rather than a tuning miss"). The `claimed` arm
is positional — a number has to sit inside the literal of the sentence whose claims account
for it — so the second sentence needed its own claim rather than a waiver. Step 6's clamped
current set that precedent. Claims 177 → 178.

## Perturbations, run

Seventeen edits, each applied to a green tree, the suite run with a **real exit code**
(launched directly at below-normal priority, not through `start`, which is exit-code-blind),
the tree restored. **All seventeen reddened**, sixteen of them on the check named before the
run. The restored tree is green.

| edit | reddened |
| --- | --- |
| prose `2 V` → `3 V` | the name tie, against `meta.name` |
| prose `12 V` → `13 V` | the derivation: six times two is not thirteen |
| operand points at `7.2` (the *next* sentence's number) | the derivation resolves to nothing — the same-sentence fence |
| operand points at `12`, the number being derived | the same, on the result-is-not-an-operand fence |
| the word operand is `fifty`, which the phrase does not contain | the rule check, on the phrase-pins-the-word fence |
| `WORD_NUMERALS` says `six` is 7 | the derivation's arithmetic (**and** the two-tables agreement check, which caught it independently) |
| prose `0.36 A` → `0.40 A` | the demand-box control |
| prose `C/20` → `C/19` | the ratio, **alone** — see below |
| prose `1.75 V` → `1.70 V` | the chemistry's `cell.v_min` |
| prose `180 mV` → `170 mV` | the span |
| the span tie reads `ocv.soc.*` instead of `ocv.volts.*` | the span, against the other column of the same table |
| prose `19.3h` → `19.4h` | the clock |
| the lesson's `until_s` moves to 69700 | the clock, from the other side: the row now renders `19.4h` |
| prose `step 1` → `step 2` | the ordinal, against where `bare-curve` sits |
| prose `7.2 Ah` → `7.5 Ah` | the chemistry's `cell.capacity_ah` |
| the second `3.3 %` claim is deleted whole | the ledger, on a number nothing accounts for |
| the operand fence is neutered | the fence's own test — see below |

Two of these are worth naming.

**`C/20` → `C/19` is the isolation, and the file-side edit would not have been.** Changing
the demand box or `capacity_ah` reddens the setting rule *and* the ratio together, so
neither could be attributed; and moving `capacity_ah` moves half the engine besides. The
prose edit reaches the ratio and nothing else. That is the `ambient_c → 40` lesson of
`docs/plans/path-derived-arm.md` applied ahead of time rather than discovered.

**The operand-must-be-accounted fence cannot be reached through the front door**, and this
is a property of the scan rather than a gap. Numbers are visited in text order, so step 22's
`2` is reached before the `12` derived from it: every edit that un-accounts the operand —
deleting its rule, breaking its phrase — reddens on the operand's own line, one number
sooner, and never reaches the assert. So the fence has a test of its own
(`a_derivation_refuses_an_operand_nothing_else_accounts_for`), built on the step's real prose
with an empty cover map, and that test was itself hand-validated: with the assert neutered it
fails with "did not panic as expected".

## The arm count is derived now

Three sentences claiming an arm was missing after it had been built is a defect this repo
shipped one slice ago (commit `5efd5a4`), and the reason nothing caught it is that the
ledger's arm tally is a **word** while the self-count machinery reads digits. Both files'
copies of that count are now derived — from **use**, not from the enum, on the same terms as
check 6's arm count: an arm no rule names is a gap rather than coverage. `tie_arm_name` is
exhaustive, so a twelfth variant does not compile until it is named.

The claims file's twin of the "quantities spelled in English" count was stale in both halves
("two of these three steps state four that way", written when three steps were ledgered and
left through two more). It cannot be derived — that needs the word scanner the ledger
deliberately has not got — so it is now a `NOT_DERIVED` waiver, which means rewording it
reddens.

## Deferred, with a price

* **`Tie::Ratio` and `Tie::Span` each have exactly one user.** That is this file's standing
  rule working (an arm with nothing to account is the `CCCV_PERIOD_S` shape), but it also
  means neither has been exercised by a second sentence with different shape — a span over a
  table whose ends are not its first and last entries, say, or a ratio whose denominator is a
  scenario field rather than a control.
* **`Tie::Derived` is product-only.** The operation is fixed because that is the identity its
  one sentence states. The day a ledgered step derives a number by another operation, this
  grows an `op` the way check 6's `[[derived]]` carries one.
* **The per-step numeral counts in `[ledger].steps`' own comments are not derived.** Each
  entry says how many numerals its step prints ("16 numbers: …") and nothing checks it, which
  is the same shape as the counts this file has already been caught by twice. Deriving them
  needs one tally per entry, which is the next self-count slice rather than this one.
* **Two of the path's twenty-four steps still carry no claim at all**, and eighteen have only
  their claimed sentences checked. Six ledgered is a beginning, not coverage.
* **Nothing here reaches step 23 or 24**, which share this step's scenario file and whose
  headline ratios (`87 times`, `sixty times`) are exactly the sibling-derived shape the new
  arm was built for. They are the cheapest next steps to ledger *because* of this slice.
