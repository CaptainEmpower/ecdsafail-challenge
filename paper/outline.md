# Paper outline — a formally-verified, completeness-rigorous ECDLP point-add

*Draft outline. Framing per `novelty-assessment.md`: lead with the rigor
methodology (machine-checked arithmetic + computed/verified completeness +
reproducibility), with the optimized secp256k1 point-addition as the case study.
Honest positioning throughout — Toffoli-competitive with a stronger correctness
guarantee, not "smallest".*

**Working title.** *Verified and Completeness-Rigorous Resource Estimation for the
secp256k1 Elliptic-Curve Discrete Log: A Reproducible Point-Addition Case Study.*

**Target.** arXiv (cs.CR / quant-ph) preprint first. Possible venue: a quantum
computing / applied-crypto workshop, or the tooling/reproducibility track of a
security venue. Not a head-to-head "we beat Google" claim.

**Abstract (≈150 words, to draft last).** Resource estimates for Shor-ECDLP on
secp256k1 have converged to `<70–90M` Toffoli / `<1200–1450` logical qubits, but
their correctness rests on sampling/fuzz and their completeness (the incomplete
affine adder) on negligibility arguments. We contribute a *verified, reproducible*
estimate of the point-addition primitive: (i) the load-bearing Solinas modular
arithmetic proved over all inputs with z3 and re-proved with bit-precise bounded
model checking bound to the production 256-bit integer type; (ii) an exactly
computed, circuit-verified treatment of the affine adder's exceptional cases,
**demonstrated end-to-end by a toy-scale discrete-log recovery** that runs the full
Shor-ECDLP with the incomplete adder and this handling; and (iii) an
emitted-and-measured (not only derived) full-ladder cost. The optimized
primitive costs `1.36M` Toffoli × `1152` qubits — under both published
point-addition bounds — and the whole pipeline is byte-reproducible.

---

## 1. Introduction
- The quantum threat to secp256k1 (Bitcoin/ECDSA); why point-addition is the inner loop.
- The gap: estimates converge, but *correctness* is by sampling/fuzz and
  *completeness* by argument. Contributions:
  1. machine-checked arithmetic bound to implementation types (z3 + Kani);
  2. exact + circuit-verified completeness (not negligibility-by-citation);
  3. emitted-and-measured full-ladder cost;
  4. a fully reproducible open pipeline.
- Explicit non-claims: not a new algorithm; not the qubit frontier; a verified
  primitive + derived/measured ladder + a discrete-log recovery **demonstrated at toy
  scale** (exact statevector), not an executed 256-bit attack.

## 2. Background & related work
- Shor-ECDLP, windowed double-and-add, the affine chord/tangent addition and its
  four exceptional branches.
- Prior estimates: Roetteler et al. 2017; Babbush et al. 2026 (arXiv:2603.28846 —
  ZK proof *of resource costs*, Fiat–Shamir fuzz); Han Luo et al. 2026
  (arXiv:2604.02311 — Proos–Zalka EEA inversion, 1333 q); Chevignard et al. 2026
  (ePrint 2026/280 — space-optimized inversion, 1098 q). Position this work as
  *orthogonal rigor*, not a competing algorithm.

## 3. The optimized point-addition circuit (case study)
- Architecture: kickmix reversible model; Cuccaro adders; division-free **Solinas**
  reduction for `p = 2²⁵⁶−2³²−977`; measurement-based uncomputation; peephole/
  constprop.
- Measured cost: `1.36M` Toffoli (executed avg/shot) × `1152` qubits; toffoli-depth
  `1.08M`; `10.22M` ops. Under both Babbush PA bounds (`≤2.7M/1175`, `≤2.1M/1425`).
- Honest note: aggressive engineering of known primitives; the *contribution* is
  the verification/completeness around it, not a novel gate identity.

## 4. Machine-checked arithmetic correctness
- **z3 layer**: Solinas `mod_add_qq` = `(acc+a) mod p` for all `acc,a∈[0,p)`, and
  the overflow ancilla uncomputes (reversibility); 22+ peephole/adder/comparator
  lemmas, `unsat` on every negation.
- **Kani layer**: bit-precise BMC of the Solinas control flow on the **real
  `alloy_primitives::U256`** against the true secp256k1 prime (`solinas_add_u256`,
  139 checks) + a small-width twin (`solinas_add_u64`) — binding the proof to the
  implementation, not a model. Negative result: division-based `sub_mod` is not
  BMC-tractable, which is itself the argument for the Solinas design.
- **Cross-validation**: the paper's own reference kickmix adders validated through an
  independent simulator; its negative controls rejected.

## 5. Completeness: from argument to computed + verified result
- The incompleteness of the affine adder (dx=0 doubling / P=−Q, ∞) and why a
  reversible superposition circuit is sensitive to it (value **and** phase).
- **Gating experiment** — measured behaviour on exceptional inputs (ancilla clean,
  output/phase corrupted → bound the amplitude).
- **∞-start removed structurally** (direct-lookup first window; circuit-demonstrated).
- **Offset-window encoding** removes the dominant zero-window ∞ term.
- **Exact end-to-end bound** `P[⋃ₖ Aₖ]` over the real 28-window two-scalar ladder
  (`≈2⁻²⁵⁰` offset / `≈2⁻¹¹` standard, `≪` Shor's ~1%) — no equidistribution
  assumption.
- **Circuit-level confirmation on real coordinates**: a reversible detector
  (`dx=0` as x-equality + ∞-sentinel tests) matches the scalar/dlog predicate on the
  whole group of several prime-order toy curves. → the negligibility argument is now
  simulation-backed at every step.
- **Demonstrated recovery (the payload, ADR 0019)**: the full two-register Shor-ECDLP,
  run by exact statevector simulation on toy prime-order curves *with the incomplete
  adder + this handling*, **recovers the secret discrete log** — complete adder
  `P_success=(n−1)/n`, offset+incomplete recovers `m`, standard encoding's zero-window
  ∞ collapses recovery. The completeness argument ends in a recovered secret, and the
  offset encoding is shown load-bearing for the *attack*, not only the amplitude bound.

## 6. From per-addition to full ECDLP (measured, not only derived)
- Windowed ladder composition; unary-iteration QROM lookup measured at `2^(w+1)−4`
  Toffoli/read (below the `3·2^w` headline).
- Full 28-window ladder **stream-emitted and counted** (no ~290 GB materialization):
  `~46–48M` Toffoli, toffoli-depth `~30M`, peak `1168` (A2); the measured totals
  consumed by the estimate. Quantum-addend register-overlap and read→add
  serialization depth measured (not assumed).
- Physical mapping: surface-code cost model → `~5 min` reaction-limited runtime,
  `~3.4M` physical qubits @ d=27 (coarse upper bound), spacetime volume.

## 7. Reproducibility
- Byte-identical build (`ops.bin` SHA pinned), `just` recipes, deterministic
  analysis suite, ADR trail, pinned toolchains. Anyone can re-derive every number.

## 8. Limitations & honest scope
- Derived/composed full-ECDLP, not an independent algorithm; not the qubit frontier;
  Toffoli×qubits is a competition metric (physical framing is the defensible one);
  completeness is a rigorous *simulation-backed argument*, not a single formal
  256-bit proof; toy-curve exhaustiveness + closed-form scale-up.

## 9. Conclusion
- The community can and should machine-check and completeness-verify these estimates;
  this is a template for doing so, with an aggressively-optimized primitive as proof
  that rigor and competitiveness are compatible.

## Appendices
- A. z3 lemma list + Kani harness statements. B. Completeness derivation + toy-curve
  parameters. C. Full metric tables. D. Reproduction commands.

## Open TODO before submission
- [x] Full-text diff vs Babbush App. A (fuzz vs proof) and the space-optimized
      papers (exceptions) — done: Babbush's ZK is *of resource costs* over a ≥99%
      fuzz sample with no completeness/point-at-infinity treatment; neither Han Luo
      (2604.02311) nor Chevignard (2026/280) machine-checks or handles exceptions.
      Items 1–2 hold and sharpen. See `novelty-assessment.md`.
- [ ] Decide authorship / competition-attribution / disclosure posture.
- [ ] Independent third-party reproduction of the byte-identical build + numbers.
- [ ] Tighten the physical cost model (currently a coarse upper bound above the
      paper's optimized <500k physical qubits).
