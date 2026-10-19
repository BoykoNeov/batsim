# Phase 12, slice D — plating taught, by a rule and as a reaction

**Status: BUILT 2026-10-19.** Plan: `phase-12-plating.md` §D, whose addendum records the
measurements taken before asking and the owner's four decisions. Harness (out of tree, not kept):
a probe crate of instruments, the scenario drafts, and a throwaway worktree the lesson was built in.

## The owner's decisions, recorded

Asked and answered on 2026-10-19 (full text in `phase-12-plating.md` §D):

1. **Stop the runaway first.** The slice's measurements found that a rest after a cold fast charge
   on the fine grid could turn a reaction cell's plated lithium into infinity and NaN. Fixed first,
   as its own note with predictions registered before the change: `plating-unconverged-runaway.md`.
2. **A plated-lithium readout beside the voltage gap.** `plated` on the page, from a new wire field.
3. **The fine grid written into the scenario files**, not left to the page's toggle.
4. **Four steps in a new chapter**, the tenth: 0 °C 1C on the threshold (the reaction file as the
   arm), its rest, 0 °C 0.3C, and −10 °C 1C.

The constraint from `dfn-cold-hold.md` holds: no constant-voltage hold on a cold charge. Every
charge here is the page's `Pulse` program, which charges for a fixed time that stops just under
4.2 V, then rests.

## What was built

* **Four scenario files**, `plating_cold_threshold`, `plating_cold_reaction`,
  `plating_colder_threshold`, `plating_colder_reaction`: one `PackConfig` with `plating` or
  `initial_temp_k` changed, which `plating_files_differ_only_where_their_headers_say` asserts. 1S1P
  LG M50, `Dfn` on `dfn::FINE_*` (40/20/40, 40 shells), isothermal, from empty, no BMS, aging on
  with the drawn fades. The first file's header carries the table below.
* **The wire: `PlatingView` gains `reversible_fraction` and `dead_fraction`** — the stores as
  shares of the chemistry's rated `capacity_ah` — and `Cells` (sim-wasm) and `/cells` (sim-server)
  gain `plated`, each cell's view, `null` for the pack where no cell runs the reaction.
  `WASM_API_VERSION` 10 → 11 and `WASM_API_MIN` with it (v9's reason: against a v10 bundle the row
  would print "no plating reaction" about a cell that runs one). `sim_server::API_VERSION` stays,
  by its exemption for additions. No snapshot change.
* **The `plated` readout row**: `0.0257 % reversible, 0.0257 % dead`, four decimals, the pack's
  largest of each; `no plating reaction` where there is none. Mirrored and pinned in
  `path_claims.rs` (`render_row`, `MIRRORED`), with two quantities, `plated_reversible_at` and
  `plated_dead_at`, and one new tie, `Tie::Celsius`, for the threshold's gate printed in the
  Celsius the prose speaks.
* **Guided-path steps 41–44** (`judged-by-a-rule`, `some-of-it-goes-back`,
  `the-charge-the-datasheet-allows`, `colder-and-billed-throughout`) in a tenth chapter, "Plating,
  by a rule and as a reaction". `dt = 5` (the plating sub-step), `rearm: true` on all four, so a
  Load stops at the mark. Every numeral is tied: 24 claims, 20 ledger rules, all four steps in the
  ledger whole (19, 13, 11 and 14 numbers) and in `spelled` at 0.
* **`crates/sim-data/tests/plating_lesson.rs`**, for the sentences with no number: the four files
  are one; the threshold is silent at exactly 0 °C on the whole 1C charge and flags it a hundredth
  of a kelvin colder; the cell rests higher while it strips because of the metal — with the rate
  constant scaled by 1e-6 the gap a minute into the rest is under a microvolt, and the reaction cell
  holds less charge, not more.
* README's guided-path paragraph, the `Start — 44 steps` button, the picker's fallback list and the
  harness's self-counts.

## The numbers, measured on the page's shape

Out-of-tree probe (the scenario loader, the `Pulse` program as `pulsePhase` runs it, 5 s steps):

| run | first `PLATING_RISK` | `soh cap` at the mark | `plated` at the mark |
| --- | --- | --- | --- |
| 0 °C 1C, threshold, 1800 s | never | 99.983 % | — |
| 0 °C 1C, reaction, 1800 s | 555 s | 99.958 % | 0.0257 % + 0.0257 % dead |
| the same, 60 s into the rest | | | 0.0166 % + 0.0257 % dead |
| 0 °C 0.3C, threshold, 9440 s | never | 99.961 % | — |
| 0 °C 0.3C, reaction, 9440 s | 8455 s | 99.960 % | 0.0008 % + 0.0008 % dead |
| −10 °C 1C, threshold, 1390 s | 5 s (every step) | 99.818 % | — |
| −10 °C 1C, reaction, 1390 s | 270 s | 99.947 % | 0.0455 % + 0.0455 % dead |

A minute into the 0 °C rest the reaction cell reads 3.878 V against the threshold cell's 3.876 V
(3.8487 against 3.8472 at two minutes, the same to the millivolt from five). Every run converged on
every step; 5 s steps reproduce 1 s ones to the digits printed (`phase-12-plating.md` §D).

## Perturbations

Predictions written into the harness before the run; each case edits one file in a throwaway copy,
runs `path_claims` and `plating_lesson`, and restores the file from the bytes read before the edit,
checked byte for byte.

| case | change | predicted to redden | reddened |
| --- | --- | --- | --- |
| **A** | step 41's arm `99.96 %` printed `99.97 %` | the literal check, the ledger | `every_claim_appears_in_its_own_step`, `every_numeral_in_a_ledgered_step_is_accounted_for` — held |
| **B** | `plating_cold_reaction` back to the threshold | the file-pair test, the gap test, the claims on its arms | `plating_files_differ_only_where_their_headers_say`, `the_resting_gap_is_the_metal`, `every_claim_matches_the_engine` — held |
| **C** | `[dfn.plating] rate_constant_m_per_s` raised 30 % | the plated-value claims alone | `every_claim_matches_the_engine` alone — held |
| **D** | `Tie::Celsius` without the kelvin offset | the ledger (the threshold's 0 °C) | `every_numeral_in_a_ledgered_step_is_accounted_for` — held (first written so it did not compile; rerun) |
| **E** | the `plated` row at three decimals | the mirror pin | `mirrored_constants_still_match_the_page` — held |
| **F** | control: a comment beside a lesson | nothing | nothing — held |

## Gates

* `cargo test --workspace --release --no-fail-fast` (below normal priority): **111 test binaries, 879 passed, 0 failed**.
  `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --all --check` clean.
* `web/pkg` rebuilt; `node tools/wasm-parity/parity.mjs`: **180/180 bit-identical** (90 runs, 90
  snapshot handoffs), the four plating files among them.
* **Walked in a real page** (`walk.mjs`): `sim-server` on its own port, headless Chrome on its own
  profile and port. The page reports `api 11 · snapshot 30`; Start shows `Start — 44 steps`; the
  chapter menu lists ten chapters, the last "Plating, by a rule and as a reaction". Each step and
  each arm stops at its mark in 14–20 s of wall clock at `dt` 5, and reads: step 41 `99.98 %` and
  `no plating reaction`, its arm `99.96 %`, `0.0257 % reversible, 0.0257 % dead` and
  `PLATING_RISK`; step 42 `3.878 V` and `0.0166 % reversible, 0.0257 % dead`, run on to
  `0.0000 % reversible, 0.0257 % dead`, the threshold arm `3.876 V`; step 43 `0.0008 % reversible,
  0.0008 % dead` and `99.96 %`, its arm `99.96 %`; step 44 `99.82 %`, its arm `99.95 %` and
  `0.0455 % reversible, 0.0455 % dead`. The picker offers every file an arm loads. No console
  errors. Chrome closed through CDP, the server stopped by the PID recorded at launch.
  **Not walked: the chapter menu by typed keys.** "10" typed into the menu in headless Chrome
  landed on chapter 1 (the select's type-ahead did not combine the two keys); the walk reached step
  41 by Next. The Phase 10 walk's single typed digit is not a two-digit chapter number.
* Step 41's `terminal` prints `4.200 V` at the mark: the cell is at 4.1996 V, under the ceiling as
  the prose says, and the panel's third decimal rounds it up. No sentence quotes that row there.

## What this slice does not do

* **No constant-voltage hold**, by the owner's decision: the defect `dfn-cold-hold.md` records stays
  open.
* **The failure to converge under the runaway stays open.** After a cold 2C or 3C charge on the fine
  grid the rest is finite and flagged, and still not a rest (`plating-unconverged-runaway.md`). A
  reader who types 2C into these steps sees a flagged, wrong curve, not NaN.
* **The sizes are placeholders'**: the rate constant, the dead share, and the threshold's fade per
  amp-hour. The −10 °C step's comparison says so in its prose.
* **No discharge after the charge.** The stripping shoulder on a following discharge — the
  laboratory signature — is named in the prose and not shown: it is about 4 mV for about a minute
  (slice C), under the panel's three decimals of a falling voltage.
