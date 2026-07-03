# Novelty assessment — is there a publishable delta?

*Working document. Assesses what this repo does that the current ECDLP
resource-estimation literature does not, to decide whether a preprint is
warranted and how to frame it. Claims about other papers below are drawn from
their **abstracts** and this repo's existing citations; a full-text pass on
Babbush et al. Appendix A and the Chevignard et al. construction is recommended
before finalizing any novelty claim.*

## The landscape (2026)

ECDLP-via-Shor resource estimation for 256-bit curves is an active, expert-heavy
area with multiple 2026 results:

| Work | Headline (256-bit) | Correctness / completeness | Formal verification |
|---|---|---|---|
| Roetteler et al. 2017 (ePrint 2017/598) | ~2330 qubits, ~1.3·10¹¹ Toffoli | negligibility argument for exceptions (the framework this repo uses) | no (analysis) |
| Babbush et al. 2026 (arXiv:2603.28846, Google QAI) | `<1200 q / <90M Toffoli` or `<1450 q / <70M` | ~99% correctness via Fiat–Shamir **fuzz** (sampling); **ZK proof validates the *estimates*, not circuit correctness** | no machine-checked circuit proofs |
| Chevignard–Fouque–Schrottenloher, EUROCRYPT 2026 (arXiv:2604.02311) | **1333 q** (prime 256-bit); `5n+4⌊log₂n⌋+O(1)` q, `O(n³)` Toffoli — a genuine *space* advance | not addressed in the abstract | not mentioned (theoretical) |
| **This repo** | PA `1.36M Toffoli × 1152 q`; derived+measured full-ECDLP `~46M Toffoli / 1168 q` | **exact** end-to-end mid-ladder bound + **circuit-level** real-coordinate exceptional detector + offset ∞-removal | **z3 + Kani**, bound to the real `alloy` U256 type |

## What is genuinely differentiating

1. **Machine-checked arithmetic, bound to the implementation types.** The flagship
   papers establish correctness by sampling/fuzz (Babbush) or leave it to analysis
   (Chevignard). This repo proves the load-bearing Solinas modular reduction and
   the peephole/adder/comparator identities over **all** inputs with z3 (22+
   lemmas, `unsat` on every negation), and re-proves the Solinas control flow with
   **Kani bit-precise BMC on the real `alloy_primitives::U256` type** against the
   actual secp256k1 prime — not an abstract model. *This is the clearest gap in the
   literature.*

2. **Completeness turned from argument into a computed+verified result.** Everyone
   in this lineage cites Roetteler-style negligibility for the incomplete affine
   adder. This repo makes it concrete: (a) a gating experiment measuring the
   circuit's actual behaviour on exceptional inputs; (b) an **exactly computed**
   end-to-end mid-ladder exceptional amplitude `P[⋃ₖ Aₖ]` over the real 28-window
   two-scalar ladder (`≈2⁻²⁵⁰` offset / `≈2⁻¹¹` standard, both `≪` Shor's ~1%);
   (c) a **reversible circuit** that detects the exceptional set on real `(x,y)`
   coordinates and is shown to match the scalar/dlog predicate on the whole group
   of several toy curves; (d) an offset-window encoding that removes the dominant
   zero-window ∞ term structurally. Nearly all prior work stops at (a-as-argument).

3. **Emitted-and-measured full-ladder cost, not only a closed form.** The full
   28-window ladder Toffoli/depth/peak are stream-emitted and counted (no
   materialization), and the measured totals are consumed by the estimate — so the
   headline is corroborated by a measurement, and the read→add serialization depth
   is measured rather than assumed.

4. **Fully reproducible, open pipeline.** Byte-identical build, `just` recipes,
   deterministic analysis suite, ADR trail. Reproducibility at this granularity is
   uncommon for resource-estimate papers.

## What is NOT a strong claim (be honest)

- **Not a new algorithm.** The point-add uses known techniques (Cuccaro, Solinas,
  Gidney unary-iteration QROM, kickmix measurement-based uncompute) tuned hard. The
  full-ECDLP figure is **derived/composed** (measured PA × the paper's ladder),
  not an independent end-to-end circuit. It does not compete with Chevignard's
  *algorithmic* space result.
- **Not the qubit-count frontier.** 1168 (A2) is above Chevignard's 1098–1333 and
  Babbush's `<1200` only on the Low-Gate side; the resident-quantum-addend port is
  measured wider still (1424–1680). The honest headline is *Toffoli-competitive with
  a much stronger correctness/completeness guarantee*, not *smallest*.
- **Toffoli×qubits is a competition metric**, not a physical cost; the defensible
  framing is the spacetime/cost-model mapping.
- **No demonstrated end-to-end attack** — a verified primitive + derived/measured
  ladder + a completeness argument, not a run.

## Recommendation

Publish, if at all, as a **methods / verified-artifact** contribution:
*"A formally-verified, completeness-rigorous, reproducible resource estimate for
the secp256k1 ECDLP point-addition primitive"* — with the aggressive optimization
as a case study and the z3+Kani+completeness rigor as the headline. Do **not** lead
with "beats the frontier." Before submission, diff the full text of Babbush App. A
(does their ZK/fuzz already amount to a completeness proof?) and Chevignard
(do they handle exceptions?) to confirm items 1–2 above hold against the full
papers, not just the abstracts.
