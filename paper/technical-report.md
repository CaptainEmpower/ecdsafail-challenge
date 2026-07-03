# A Verified, Completeness-Rigorous, Reproducible Resource Estimate for the secp256k1 ECDLP Point-Addition Primitive

**Technical report — draft.** *Authors: TBD. Version: draft, {date TBD}.*
This report accompanies the `CaptainEmpower/ecdsafail-challenge` repository; every
number below is produced by a deterministic run of the code it describes (see
§Reproducibility). It is a *cost estimate with a machine-checked correctness and a
simulation-backed completeness layer* (whose pipeline is demonstrated to recover the
discrete log **at toy scale**), not an executed 256-bit attack.

## Abstract

Quantum resource estimates for Shor's algorithm on the secp256k1 elliptic-curve
discrete-log problem (ECDLP) — the primitive securing Bitcoin/ECDSA — have
converged to roughly `<70–90M` Toffoli gates and `<1200–1450` logical qubits.
Their reported correctness, however, rests on sampling/fuzz and their treatment of
the *incomplete affine addition formula* on negligibility arguments. We present a
resource estimate of the ECDLP **point-addition** primitive whose arithmetic
**integer core** is machine-checked, whose completeness is a **computed +
circuit-confirmed** argument (exact at toy scale, an analytic union bound at attack
scale), and whose every figure is **byte-reproducible**. The optimized
primitive costs **1,364,230 Toffoli × 1,152 qubits** — under both Babbush et al.
point-addition operating points — and composes to a measured full-ECDLP cost of
**~46M Toffoli / 1168 qubits**. The load-bearing modular arithmetic is proved over
all inputs (z3, at production 256/257-bit width) and re-proved with bit-precise
bounded model checking on the real `alloy_primitives::U256` type (Kani) — covering
the **plain** modular-reduction identity, not the emitted measurement-based adder,
which the 9024-shot sample covers. The affine adder's exceptional cases are bounded
(exactly at toy scale), confirmed by a reversible detector over real coordinates,
and — the payload — shown **sufficient at toy scale** by a demonstrated end-to-end
discrete-log recovery (exact statevector Shor-ECDLP that recovers the secret using a
model of the incomplete adder + this handling). We position this as a *rigor and
reproducibility* contribution orthogonal to the algorithmic frontier, not a smaller
estimate.

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

Against the Babbush et al. 2026 point-addition operating points — Low-Qubit
`≤2.7M Toffoli / 1175 q`, Low-Gate `≤2.1M / 1425 q` — the measured `1.36M / 1152`
is under **both**. *(Provenance note: Babbush et al.'s **public** headline is the
full-ECDLP totals `<1200 q/<90M` and `<1450 q/<70M`; these per-point-addition
Pareto points are the challenge's reference numbers for the same line of work.
Pin the exact source — paper table vs. organizer-supplied — before submission;
tracked as referee finding F7, issue #61.)*

## 2. Contributions

1. **Machine-checked arithmetic (integer core), bound to implementation types.**
   The Solinas division-free reduction for `p = 2²⁵⁶−2³²−977` is proved to compute
   `(acc+a) mod p` for all `acc,a ∈ [0,p)` and to uncompute its overflow ancilla
   (reversibility) via z3; the peephole/adder/comparator lemmas are `unsat` on
   negation, the adder/comparator recurrences at production 256/257-bit width
   (ADR 0024). The same control flow is re-proved with **Kani** bit-precise bounded
   model checking on the real `alloy_primitives::U256` type against the true
   secp256k1 prime. *Scope (referee F1/F2): the proofs model the **plain**
   `mod_add_qq` integer identity — the Kani harness is a hand-written
   re-implementation, not the gate-emitting builder, and the scored circuit's hot
   path emits the **`_fast` measurement-based** variant (`hmr`/`cz_if`) whose phase
   logic is validated by the 9024-shot sample, not by these proofs. The guarantee
   is an integer-core guarantee, not a gate-level proof of the emitted circuit.*
2. **Completeness: computed, circuit-confirmed, and demonstrated at toy scale —
   sharpening the argument.** The incomplete affine adder's exceptional cases are
   (a) measured on crafted inputs; (b) removed structurally where amplitude-1
   (∞-start via direct-lookup; zero-window ∞ via an offset encoding); (c) **bounded
   exactly at toy scale** — and by the analytic union bound (`28·2/n`, the
   equidistribution value) at attack scale, where the exact convolution is
   infeasible (referee F5); (d) confirmed by a **reversible detector on real (x,y)
   coordinates** matching the scalar/dlog predicate on the whole group of several
   prime-order toy curves; and (e) **demonstrated end-to-end at toy scale**: the full
   two-register Shor-ECDLP, run by exact statevector simulation on toy prime-order
   curves, **recovers the secret discrete log** (complete adder `P_success=(n−1)/n`;
   offset+incomplete recovers `m`; standard encoding's zero-window ∞ collapses
   recovery — so the handling is load-bearing for the attack, not only the
   amplitude bound). *Scope (referee F4): the recovery oracle is a **Python model**
   of the affine adder (chord-only, `inv(0):=0`), not the scored circuit; the model
   is phase-clean, whereas the scored circuit measures probabilistic phase garbage
   on `dx=0` (a). So the sharp claim is "sufficient at toy scale with a model
   adder"; the exact/asymptotic **amplitude bound** (c) — which needs only the
   exceptional amplitude, not the phase — is the load-bearing completeness result.*
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
- Demonstrated attack (toy-scale recovery): `analysis/verify/shor_ecdlp_recovery.py`.
- Full-ladder measurement: `src/point_add/{ladder_full,ladder_stream}.rs`,
  `analysis/ladder_measured.json`, `analysis/ecdlp_estimate.py`.
- Design rationale: `analysis/adr/` (ADRs 0001–0021), `analysis/scientific-value.md`.

## 4. Reproducibility

```bash
just build      # -> ops.bin  (SHA-256 pinned; byte-identical)
just score      # -> score.json  (9024-shot correctness/reversibility/phase scoring)
just depth      # -> depth.json
just analysis   # 12-stage suite: z3 proofs + completeness + toy-attack recovery + cost model + estimate
just recover    # end-to-end Shor-ECDLP dlog recovery on toy curves (ADR 0019)
just kani       # bit-precise proofs on the real alloy U256 type
```

Analysis suite requires Python 3.11+ (`pip install -r analysis/requirements.txt`,
pinned) and `just`. The build is byte-identical across runs (a fixed `ops.bin`
SHA-256); all reported numbers follow deterministically.

## 5. Limitations (honest scope)

- The full-ECDLP figure is **derived/composed** (measured point-add × the source
  paper's windowed-ladder structure), not an independent end-to-end circuit or a new
  algorithm; it does not compete with the space-optimized algorithmic results
  (Chevignard et al. 2026, **1193 qubits for P-256** — their 1098 is the P-224
  figure; Han Luo et al. 2026, 1333 qubits).
- `1168` qubits (paper bound A2) is not the qubit frontier; that bound is not a
  figure this repo's circuit independently realizes — the faithful
  resident-quantum-addend port is measured wider (1424–1680), above the P-256
  frontier. The honest headline is
  *Toffoli-competitive with a stronger correctness/completeness guarantee*.
- Toffoli×qubits is a competition figure of merit; the physical cost model is a
  coarse upper bound (above the source paper's optimized `<500k` physical qubits).
- Completeness is a rigorous **simulation-backed argument** (exact bound + circuit
  confirmation at every component + a **demonstrated discrete-log recovery**), not a
  single formal machine-checked proof of the whole 256-bit attack. The recovery is at
  **toy scale** (exact statevector on curves of order 19/29/41); the 256-bit attack
  remains a derived/measured cost estimate, bridged by toy-curve exhaustiveness +
  closed-form scale-up, not an executed run.

## 6. Related work

Roetteler et al. 2017 (ePrint 2017/598); Babbush et al. 2026 (arXiv:2603.28846,
Google Quantum AI — zero-knowledge proof *of resource costs*, ≥99% Fiat–Shamir fuzz
correctness, no explicit completeness treatment); Han Luo et al. 2026
(arXiv:2604.02311, Tsinghua/Peking — space-optimized Proos–Zalka EEA inversion, 1333
qubits); Chevignard–Fouque–Schrottenloher, EUROCRYPT 2026 (ePrint 2026/280 —
space-optimized inversion, 1193 qubits for P-256 — 1098 qubits is their P-224
figure — the current qubit frontier). This work is
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
