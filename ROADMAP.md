# Roadmap

Tracked, actionable work for this repo. Each item links to its GitHub issue and
to the in-repo docs where the detail and rationale live. This file is an index,
not a second source of truth — decisions live in `analysis/adr/`, and the honest
list of what the analysis does **not** yet cover lives in
`analysis/scientific-value.md` (§2 "Key limitations", §Scope/honesty) and the
external referee review `paper/REVIEW.md`.

## Open

### Challenge score
- [ ] **Reduce Toffoli / peak-qubit liveness** —
  [#6](https://github.com/CaptainEmpower/ecdsafail-challenge/issues/6).
  Current ~1.57e9 (1,364,230 Toffoli × 1,152 qubits) already beats the source
  paper's Low-Qubit point-addition operating point on all three axes; both
  factors are heavily hand-tuned. High effort, uncertain payoff. Editable path:
  `src/point_add/` only, `ops.bin` re-scored per change.

### Code health
- [ ] **Split `point_add` files into SRP modules of ≤300 LOC** —
  [#10](https://github.com/CaptainEmpower/ecdsafail-challenge/issues/10).
  Pure code movement, validated byte-identical (`ops.bin` SHA `f30d8365…`) after
  every split. Incremental, one file per PR.

### Analysis / rigor — remaining honest stretches
These are the gaps the analysis layer still names as open. None is load-bearing
for a current claim; each is an optional strengthening.
- [ ] **Symbolic proof of the *composed* point-add end-to-end.** The z3/Kani
  layer proves the algebraic lemmas each optimization depends on, and ADR 0027
  now proves the emitted `_fast` adder's measurement-based uncompute — but not a
  symbolic execution of the whole ~10M-op emitted point-add against the reference
  group law (does not scale in either solver). The composition into a full
  point-add stays guarded by the 9024-shot sample.
  Detail: `analysis/scientific-value.md` §1 Scope/honesty + §4.
- [x] **(Optional) Kani harness bound to the *emitter*, not a copy.** Done —
  ADR 0030. `src/point_add/mbuc_kani.rs` drives the real `B` builder + real
  `Simulator` for `cuccaro_add_fast`, proving functional/clean/phase-clean over
  all inputs and all measurement outcomes at small width (`#[kani::proof]`), with
  an exhaustive `#[cfg(test)]` shadow at widths 2/3/4 in `cargo test`. Honest
  scope: binds to the real emitter/types at small width — not production 256
  (BMC-intractable; that width is the z3 layer's job, ADR 0027). Detail: ADR 0030,
  `paper/REVIEW.md` F2.
- [ ] **Pre-submission: pin the exact source for the PA Pareto operating
  points.** The `2.7M/1175` and `2.1M/1425` point-addition numbers are used as
  challenge reference figures; Babbush et al.'s public abstract reports only
  full-ECDLP totals. Confirm the exact provenance before any external writeup.
  Detail: ADR 0025 (F7), `paper/REVIEW.md` F7.

## Done

The scientific-rigor arc and the external referee remediation are complete; the
detail lives in the ADR trail (`analysis/adr/`, index at
`analysis/adr/README.md`). Headline milestones:

- **Formal correctness of the arithmetic core** — z3 over all inputs + Kani on
  the real `alloy` U256 type, now at production 256/257-bit width
  ([#58](https://github.com/CaptainEmpower/ecdsafail-challenge/issues/58),
  ADR 0024), plus a z3 proof of the emitted `_fast` adder's measurement-based
  uncompute ([#57](https://github.com/CaptainEmpower/ecdsafail-challenge/issues/57),
  ADR 0027). ADR 0001–0005, 0024, 0027.
- **Emitter-bound proofs — the copy↔emitter gap (referee F2) closed across the
  arithmetic core.** The emitted `cuccaro_add_fast` adder is proved in z3 over its
  op-stream (ADR 0027) and by a Kani harness driving the real `B` builder +
  `Simulator` (ADR 0030); the emitted `mod_add_qq` **Solinas reduction** in z3 at
  production 256-bit width (ADR 0031); and the **scored `_fast` modular wrappers**
  (`mod_add_qq_fast`/`mod_sub_qq_fast`/`mod_double_inplace_fast` — the 58 hot-path
  calls) over their emitted, HMR-carrying gates (ADR 0032) — the measurement-based
  Solinas fold verified in context. ADR 0032 also surfaced a lazy-reduction in
  `mod_double_inplace_fast` (unreduced on a ~2³¹ input window the sampled test
  misses; congruent mod p, harmless downstream, now disclosed). The step-for-step
  model (ADR 0024) and the Kani integer twin become independent cross-checks. All
  reuse the `proof_toolkit` methodology (ADR 0028/0029).
- **Tier B — the full ECDLP ladder emitted + measured end-to-end**
  ([#4](https://github.com/CaptainEmpower/ecdsafail-challenge/issues/4)):
  windowed QROM lookup cost measured (ADR 0010), full ladder stream-emitted and
  counted (ADR 0011/0017), quantum-addend point-add sim-verified (ADR 0014),
  consumed by `ecdlp_estimate.py`.
- **Adder completeness** ([#5](https://github.com/CaptainEmpower/ecdsafail-challenge/issues/5)):
  exact exceptional rate measured (ADR 0008), zero-window ∞ removed structurally
  (ADR 0009/0015), exact end-to-end bound (ADR 0016), reversible real-coordinate
  detector (ADR 0018), and toy-scale discrete-log **recovery** demonstrated
  (ADR 0019).
- **Path B — complete arithmetic at toy scale**
  ([#48](https://github.com/CaptainEmpower/ecdsafail-challenge/issues/48)):
  reversible modular inverse (ADR 0020), complete λ-division affine point-add
  handling all exceptional cases (ADR 0021), and the fully gate-level toy Shor
  capstone recovering the secret dlog (ADR 0022).
- **Independent referee review** ([#62](https://github.com/CaptainEmpower/ecdsafail-challenge/issues/62),
  `paper/REVIEW.md`, ADR 0023): score + suite reproduced; findings F1–F8
  remediated (ADR 0024 F3, ADR 0025 F6/F7/F8, ADR 0026 F1/F2/F4/F5 framing,
  ADR 0027 F1/F2 proof).
- **Reproducible analysis env** — uv-managed, hash-pinned
  ([#51](https://github.com/CaptainEmpower/ecdsafail-challenge/issues/51)); build
  warnings addressed ([#7](https://github.com/CaptainEmpower/ecdsafail-challenge/issues/7)).
- **Reusable proof toolkit** ([#70](https://github.com/CaptainEmpower/ecdsafail-challenge/issues/70),
  ADR 0028 scope + ADR 0029 build): the verification *methodology* — the
  generalized z3 `src/sim.rs` op-stream replayer — extracted from ADR 0027 into
  `analysis/verify/proof_toolkit/` (`just toolkit` self-test), with
  `mbuc_phase_correction.py` refactored onto it byte-identical. The
  score-specialized primitives are deliberately **not** carved out (ADR 0028); a
  clean-room primitive crate stays deferred until a second consumer exists.

---

**Provenance.** This repo is a solution to the challenge from Babbush et al.
2026, *Securing Elliptic Curve Cryptocurrencies against Quantum Vulnerabilities*
(arXiv:2603.28846v2); see `analysis/adr/0003-*` and `0004-*`. Remotes: `origin`
is the working fork, `upstream` is the canonical challenge repo.
