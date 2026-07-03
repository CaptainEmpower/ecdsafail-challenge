# A Verified, Completeness-Rigorous, Reproducible Resource Estimate for the secp256k1 ECDLP Point-Addition Primitive

**Technical report — draft.** *Authors: TBD. Version: draft, {date TBD}.*
This report accompanies the `CaptainEmpower/ecdsafail-challenge` repository; every
number below is produced by a deterministic run of the code it describes (see
§Reproducibility). It is a *cost estimate with a machine-checked correctness and a
simulation-backed completeness layer*, not a demonstrated end-to-end attack.

## Abstract

Quantum resource estimates for Shor's algorithm on the secp256k1 elliptic-curve
discrete-log problem (ECDLP) — the primitive securing Bitcoin/ECDSA — have
converged to roughly `<70–90M` Toffoli gates and `<1200–1450` logical qubits.
Their reported correctness, however, rests on sampling/fuzz and their treatment of
the *incomplete affine addition formula* on negligibility arguments. We present a
resource estimate of the ECDLP **point-addition** primitive whose correctness and
completeness are, respectively, **machine-checked** and **exactly computed and
circuit-verified**, and whose every figure is **byte-reproducible**. The optimized
primitive costs **1,364,230 Toffoli × 1,152 qubits** — under both published
point-addition bounds — and composes to a measured full-ECDLP cost of **~46M
Toffoli / 1168 qubits**. The load-bearing modular arithmetic is proved over all
inputs (z3) and re-proved with bit-precise bounded model checking bound to the
production 256-bit integer type (Kani); the affine adder's exceptional cases are
bounded exactly on the real 28-window ladder and confirmed by a reversible detector
over real coordinates. We position this as a *rigor and reproducibility*
contribution orthogonal to the algorithmic frontier, not a smaller estimate.

## 1. Result summary (measured)

| Quantity | Value | Basis |
|---|---|---|
| Point-add Toffoli (score) | **1,364,230** | executed avg / shot (`score.json`) |
| Point-add qubits (score) | **1,152** | max allocated id + 1 |
| Score (Toffoli × qubits) | **1,571,592,960** | lower is better |
| Point-add Toffoli-depth | 1,077,263 | `depth.json` |
| Point-add ops | 10,221,377 | byte-identical build |
| Windowed QROM read | 2^(w+1)−4 = 131,068 Toffoli | measured (unary iteration) |
| Full ladder (w=16, 28 adds) | ~47.8M reversible / ~46.0M MBUC Toffoli; depth ~30.2M; peak 1168 | stream-emitted + counted |
| Mid-ladder exceptional amplitude | `≈2⁻²⁵⁰` (offset) / `≈2⁻¹¹` (std) | exact, `≪` Shor's ~1% |
| Physical (coarse) | ~5 min runtime, ~3.4M physical qubits @ d=27 | surface-code cost model |

Against the published point-addition bounds (Babbush et al. 2026): Low-Qubit
`≤2.7M Toffoli / 1175 q`, Low-Gate `≤2.1M / 1425 q` — the measured `1.36M / 1152`
is under **both**.

## 2. Contributions

1. **Machine-checked arithmetic, bound to implementation types.** The Solinas
   division-free reduction for `p = 2²⁵⁶−2³²−977` is proved to compute
   `(acc+a) mod p` for all `acc,a ∈ [0,p)` and to uncompute its overflow ancilla
   (reversibility) via z3; 22+ peephole/adder/comparator lemmas are `unsat` on
   negation. The same control flow is re-proved with **Kani** bit-precise bounded
   model checking on the real `alloy_primitives::U256` type against the true
   secp256k1 prime — binding the proof to the code, not a model.
2. **Completeness: computed and circuit-verified, not argued.** The incomplete
   affine adder's exceptional cases are (a) measured on crafted inputs; (b) removed
   structurally where amplitude-1 (∞-start via direct-lookup; zero-window ∞ via an
   offset encoding); (c) **bounded exactly** end-to-end on the real 28-window
   two-scalar ladder; and (d) confirmed by a **reversible detector on real (x,y)
   coordinates** matching the scalar/dlog predicate on the whole group of several
   prime-order toy curves.
3. **Emitted-and-measured full-ladder cost.** The full ladder is stream-emitted and
   counted (no materialization), and the measured totals are consumed by the
   estimate — corroborating the derived headline and measuring the read→add
   serialization depth rather than assuming it.
4. **Reproducibility.** Byte-identical build, deterministic analysis suite, pinned
   toolchains, and an ADR trail; see §Reproducibility.

## 3. Method (pointers into the artifact)

- Circuit + score: `src/point_add/`, `benchmark.sh`, `src/bin/eval_circuit.rs`.
- z3 proofs: `analysis/verify/solinas_reduction.py`, `peephole_identities.py`.
- Kani proofs: `src/kani_proofs.rs` (`solinas_add_u256` / `_u64`).
- Reference-circuit cross-validation: `analysis/verify/validate_reference_adders.py`.
- Completeness: `analysis/completeness_argument.md`,
  `verify/{completeness_collision_rate,direct_lookup_init,offset_window_encoding,mid_ladder_bound}.py`,
  `src/point_add/ec_exceptional.rs`.
- Full-ladder measurement: `src/point_add/{ladder_full,ladder_stream}.rs`,
  `analysis/ladder_measured.json`, `analysis/ecdlp_estimate.py`.
- Design rationale: `analysis/adr/` (ADRs 0001–0018), `analysis/scientific-value.md`.

## 4. Reproducibility

```bash
just build      # -> ops.bin  (SHA-256 pinned; byte-identical)
just score      # -> score.json  (9024-shot correctness/reversibility/phase scoring)
just depth      # -> depth.json
just analysis   # 11-stage suite: z3 proofs + completeness + cost model + estimate
just kani       # bit-precise proofs on the real alloy U256 type
```

Analysis suite requires Python 3.11+ (`pip install -r analysis/requirements.txt`,
pinned) and `just`. The build is byte-identical across runs (a fixed `ops.bin`
SHA-256); all reported numbers follow deterministically.

## 5. Limitations (honest scope)

- The full-ECDLP figure is **derived/composed** (measured point-add × the source
  paper's windowed-ladder structure), not an independent end-to-end circuit or a new
  algorithm; it does not compete with the space-optimized algorithmic results
  (Chevignard et al. 2026, 1098 qubits; Han Luo et al. 2026, 1333 qubits).
- `1168` qubits (paper bound A2) is not the qubit frontier; a faithful
  resident-quantum-addend port is measured wider (1424–1680). The honest headline is
  *Toffoli-competitive with a stronger correctness/completeness guarantee*.
- Toffoli×qubits is a competition figure of merit; the physical cost model is a
  coarse upper bound (above the source paper's optimized `<500k` physical qubits).
- Completeness is a rigorous **simulation-backed argument** (exact bound + circuit
  confirmation at every component), not a single formal machine-checked proof of the
  whole 256-bit attack; toy-curve exhaustiveness + closed-form scale-up.

## 6. Related work

Roetteler et al. 2017 (ePrint 2017/598); Babbush et al. 2026 (arXiv:2603.28846,
Google Quantum AI — zero-knowledge proof *of resource costs*, ≥99% Fiat–Shamir fuzz
correctness, no explicit completeness treatment); Han Luo et al. 2026
(arXiv:2604.02311, Tsinghua/Peking — space-optimized Proos–Zalka EEA inversion, 1333
qubits); Chevignard–Fouque–Schrottenloher, EUROCRYPT 2026 (ePrint 2026/280 —
space-optimized inversion, 1098 qubits, the current qubit frontier). This work is
orthogonal: it adds machine-checked correctness, computed/verified completeness, and
full reproducibility to an aggressively optimized primitive.

## How to cite

```bibtex
@techreport{ecdsafail-secp256k1-pointadd,
  title  = {A Verified, Completeness-Rigorous, Reproducible Resource Estimate
            for the secp256k1 ECDLP Point-Addition Primitive},
  author = {TBD},
  year   = {2026},
  note   = {Technical report, CaptainEmpower/ecdsafail-challenge},
  url    = {https://github.com/CaptainEmpower/ecdsafail-challenge}
}
```
