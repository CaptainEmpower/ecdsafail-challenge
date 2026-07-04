# ADR 0033 — Prove the constant-propagation peephole's soundness premise

**Status:** Accepted — the affine-domain soundness lemmas are proved in
`analysis/verify/peephole_identities.py` (z3, `38/38`), and the real `xor_set` is bound
to symmetric-difference-over-canonical-form by an exhaustive `#[cfg(test)]` test in
`src/point_add/trailmix_ludicrous/constprop.rs` (runs in `cargo test`/CI). Retires the
last *argued-and-sampled* premise in a **score-affecting** optimization.
**Date:** 2026-07-04

## Context

`constprop.rs` is a peephole that **mutates the scored circuit**: where it can prove two
`CCX` controls are always equal / always complementary / constant, it folds the Toffoli
to a `CX` or drops it — removing *counted* Toffoli gates, so both correctness **and the
score** depend on those claims. It proves the control relationship with a GF(2)
**affine-form** dataflow: each qubit is tracked as `eval(p, c; x) = c ⊕ (⊕_i p_i·x_i)` —
a characteristic vector `p` of "fresh" input variables plus a const bit `c` — combined
across gates with `xor_set` (symmetric difference).

The completeness-critic pass found this to be the highest-value remaining gap. The
peephole *identities* are machine-proved (`peephole_identities.py`, e.g.
`FoldEqualCtrls: a=b ⇒ CCX(a,b,t)=t⊕a`), but the *premise* that fires them — that the
tracker's equal/complement/constant claim really holds on **every** basis state — was, in
the repo's own words (`scientific-value.md` §1b), *"the standard linearity argument …
corroborated by the empirical `CONSTPROP_VERIFY` pass."* Argued in a comment, checked by
sampling — the one such premise left on a transform that changes `ops.bin`.

## Decision

Discharge the premise with a two-layer proof, matching the repo's z3 + real-code pattern.

**1. The affine domain is sound (z3, `peephole_identities.py`).** Model an affine form as
`(char-vector p, const c)` and prove, universally over all forms and basis states
`x` (widths N ∈ {4,8,16}):

- **XOR-linearity** — `eval(p_t ⊕ p_c, c_t ⊕ c_c; x) == eval(p_t,c_t;x) ⊕ eval(p_c,c_c;x)`:
  the `xor_set`/`cst ^=` transfer used by `CX` and the equal-control fold is GF(2)-linear
  on `eval`, so the maintained invariant *concrete = eval* is preserved by those gates.
- **FoldEqualCtrls premise** — `set(a)==set(b) ∧ cst(a)==cst(b) ⇒ a==b` on every `x`.
- **DropComplementCtrls premise** — `set(a)==set(b) ∧ cst(a)≠cst(b) ⇒ a==¬b` (so `a&b=0`).
- **DropZeroCtrl premise** — an empty set ⇒ the qubit is the constant `c`.

These say: *given* that two qubits carry the same (resp. complementary / empty) affine
form, their concrete values are equal (resp. opposite / constant) on all inputs — exactly
what the folds assume.

**2. The real `xor_set` implements the domain (exhaustive Rust test, `cargo test`).** The
z3 lemmas assume `xor_set` computes *symmetric difference* and that the `Vec<u32>`
representation stays **canonical** (strictly increasing) — the latter is what lets the
folds use `af.set[a] == af.set[b]` (`Vec` equality) as *set* equality. A `#[cfg(test)]`
test drives the real `xor_set` over **every** pair of subsets of an 8-element universe and
asserts (a) membership equals symmetric difference on every element and (b) the output is
canonical. The merge is id-value-agnostic (only ordering matters), so a representative
8-id universe exercises every branch — exhaustive for the algorithm.

Together, the tracker's equal/complement/constant claims are sound on every basis state:
`xor_set` really is symmetric-difference-over-a-canonical-form (Rust), and that domain's
equalities imply concrete equalities (z3). The remaining transfer steps are trivial (`X`
flips `c`; `Swap` exchanges entries; `R` sets the constant 0; any not-provably-executed
write or non-linear `CCX` **collapses the target to a fresh, globally-unique variable** —
a sound over-approximation that asserts no relation).

## As built

`just peephole` now reports `38/38` lemmas (was 26 + the 12 affine-soundness lemmas);
`cargo test` runs `constprop::affine_soundness::xor_set_is_symmetric_difference_and_canonical`
(65 536 subset pairs, <0.1 s). Both are in the standard per-PR CI — no new heavy stage.

## Consequences

- **Closes the last argued+sampled premise on a score-affecting transform.** The constprop
  folds/drops that remove counted Toffoli gates now rest on proof, not on the linearity
  argument + `CONSTPROP_VERIFY` sampling. `scientific-value.md` §1b is updated accordingly.
- **Low risk realized as proof.** GF(2) linear tracking is textbook and was correctly
  applied — this converts "almost certainly fine" into "checked", which is the point of
  the whole rigor arc.
- **Both layers land in the fast per-PR CI** (z3 lemma set + a sub-0.1 s `cargo test`),
  unlike the heavy emitted-gate proofs — so this premise is verified on every push.
- **Honest scope.** This proves the *soundness* of the affine relation-claims (no false
  equality), i.e. the peephole never applies an unsound fold. It is not a proof that the
  analysis is maximally *precise* (it may conservatively miss folds) — precision is not a
  correctness property and needs no proof.
- **Isolation ([ADR 0001](0001-analysis-layer-isolated-from-score.md)).** The z3 script is
  analysis-layer; the Rust check is `#[cfg(test)]`. The scored circuit is byte-identical
  (`ops.bin` SHA `f30d8365c1235002`).
