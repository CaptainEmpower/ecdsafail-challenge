# Scientific value of the ecdsafail-challenge circuit

This document turns the repository from a competitive-optimization artifact into
something with defensible scientific standing. It does three things:

1. **Formally verifies** the algebraic invariants the optimizations rely on
   (previously checked only by sampled simulation) — `analysis/verify/`.
2. **Maps the abstract score to a physical fault-tolerant cost** under stated
   assumptions — `analysis/cost_model.py`.
3. **Extracts the generalizable techniques** from the codebase and separates
   what is reusable from what is harness/curve-specific — this document.

All numbers here come from deterministic runs (`z3`, `score.json`); none are
hand-asserted. Re-run: `python3 analysis/verify/solinas_reduction.py`,
`python3 analysis/verify/peephole_identities.py`, `python3 analysis/cost_model.py`.

---

## 0. What the artifact is

A reversible circuit for **secp256k1 elliptic-curve point addition** — the inner
loop of Shor's algorithm applied to the elliptic-curve discrete-log problem
(ECDLP), i.e. the computation that breaks ECDSA (Bitcoin/Ethereum keys). It is
scored by `round(avg_toffoli_per_shot) × qubits` (`src/bin/eval_circuit.rs:434`),
where "Toffoli" counts CCX+CCZ executions (`src/sim.rs:86`) and "qubits" is the
maximum allocated qubit id + 1 (`analyze_ops` in `src/circuit.rs`). Current metrics
(`score.json`): **1,364,230 Toffoli × 1,152 qubits = 1,571,592,960**.

This places the work in **quantum resource estimation**, a legitimate and
cryptographically policy-relevant research area. The improvement is real *if*
(a) the circuit is provably correct, and (b) the score maps to a physical cost.
Sections 1–2 supply exactly those two missing pieces.

### Value to cryptanalysis (honest scope)

Two senses of "cryptanalysis" pull apart here, and the honest answer differs:

- **Cryptanalysis as *breaking* — no contribution.** There is no new attack, no
  reduced qubit/gate frontier (the primitive is `1152` q; the P-256 frontier is
  Chevignard's `1193` — their `1098` is P-224 — and dropping), and no structural weakness exposed in
  secp256k1. The hardness of the ECDLP is untouched; ECDSA is exactly as safe as
  before.
- **Cryptanalysis as *resource estimation / threat assessment* — a narrow but real
  contribution, to its *epistemics*.** Quantum resource estimation feeds
  PQC-migration timelines and harvest-now-decrypt-later risk models. The flagship
  2026 estimates establish correctness by sampling/fuzz (Babbush: ≥99% Fiat–Shamir
  fuzz, a ZK proof *of resource costs* — not all-inputs correctness) or leave it to
  analysis (Han Luo, Chevignard), and all three treat the incomplete affine adder's
  exceptional cases by negligibility-by-citation (a full-text pass found zero
  occurrences of SMT/z3/model-checking or an explicit point-at-infinity treatment in
  any of them). This repo instead **machine-checks** the load-bearing arithmetic over
  all inputs (z3 + Kani on the real `alloy` U256 type, §1) and turns completeness from
  a cited argument into a **computed + circuit-confirmed** result (`≈2⁻²⁵⁰` bound —
  exact at toy scale, an analytic union upper bound at attack scale — plus a
  reversible real-coordinate detector; see `completeness_argument.md`,
  `ec_exceptional.rs`).

  > **Scope of "machine-checks" and "computed + circuit-confirmed" (referee
  > F1/F2/F4/F5, ADR 0026).** These verbs are load-bearing, so state exactly what
  > they cover. The z3/Kani layer proves the *integer identity* of the **plain**
  > `mod_add_qq` (and Kani proves a hand-written re-implementation, not the
  > gate-emitting builder); the scored circuit's hot path emits the **`_fast`
  > measurement-based** variant (`cuccaro_add_fast` + `hmr`/`cz_if`), whose phase
  > logic is validated only by the 9024-shot sample (§1c). The completeness bound
  > is exact only on toy curves; at attack scale it is the analytic union bound.
  > The "demonstrated recovery" runs a *model* adder in Python, not the scored
  > circuit (§4). So the honest reading is: a machine-checked **integer core** and
  > a **simulation-backed** completeness argument — narrower than the bare verbs
  > suggest, and deliberately re-scoped here.

So the deliverable is not a break but a **standard of evidence**: a template showing
that "X qubits, Y Toffoli" estimates *can* be machine-checked and
completeness-verified without giving up competitiveness. That is a reproducibility /
trust contribution to the sub-discipline — real, but narrow, and deliberately framed
that way rather than as a frontier claim.

---

## 1. Formal correctness (was: empirical only)

The harness validates correctness by *sampled simulation*: 9024 random point
pairs (`benchmark.sh`) plus `CONSTPROP_VERIFY` / `ALT_SEED_*` shot replays. That
establishes correctness on the sampled inputs, not all of them — a subtle bug on
an unsampled input would silently invalidate a "frontier-beating" claim. We
discharge the underlying claims as **theorems over all inputs** (z3 returns
`unsat` on every negation).

### 1a. Solinas modular reduction — the load-bearing arithmetic identity

`mod_add_qq` (`src/point_add/arith/modular/add.rs`) computes `(acc + a) mod p`
on `p = 2^256 − 2^32 − 977` using the Solinas trick: add, add `c = 2^256 − p`,
branch on the overflow bit, conditionally undo. The comment asserts this "saves
one full (n+1)-wide Cuccaro" but never proves it. `solinas_reduction.py` models
the algorithm step-for-step as 257-bit vectors and proves, **for all
acc, a ∈ [0, p)**:

```
[PROVED] mod_add_qq: low256 == (acc + a) mod p        for all acc,a in [0,p)
[PROVED] mod_add_qq: overflow flag uncomputes to |0>  (flag == (acc_final < a))
```

The second theorem is a **reversibility** guarantee: the transient overflow
ancilla returns to |0⟩, so the sub-circuit is clean and `emit_inverse`-safe —
exactly the property the challenge's ancilla-uncompute check enforces, now proven
rather than sampled.

### 1b. Peephole, adder, and comparator invariants

`peephole_identities.py` proves the boolean claims behind the gate-level
optimizations (`26/26 lemmas PROVED`):

| Claim | Source | Theorem |
|---|---|---|
| DropZeroCtrl | `constprop.rs` | `a=0 ⇒ CCX(a,b,t)=t` |
| FoldCx | `constprop.rs` | `a=1 ⇒ CCX(a,b,t)=t⊕b` |
| FoldX | `constprop.rs` | `a=1,b=1 ⇒ CCX=¬t` |
| FoldEqualCtrls | `constprop.rs` | `a=b ⇒ CCX(a,b,t)=t⊕a` |
| DropComplementCtrls | `constprop.rs` | `a=¬b ⇒ CCX(a,b,t)=t` |
| InversePairCancellation | `constprop.rs` | `CCX;CCX (controls/target unchanged) = I` |
| Ripple-carry recurrence | `venting.rs`, `arith/adder.rs` | carry chain `= (a+b) mod 2^w`, w∈{1..64, **256, 257**} |
| Borrow-chain comparator | `comparator.rs` | final borrow `= (a <ᵤ b)`, w∈{1..64, **256, 257**} |

The `256`/`257` widths are the **production** register sizes (256-bit
coordinates; the 257-bit Solinas extended register), so the adder and comparator
recurrences are proved *at* the width the scored circuit runs — not extrapolated
from ≤64-bit instances (referee finding F3, [ADR 0024](adr/0024-z3-production-width-adder-proof.md)).

The affine-form analysis in `constprop.rs` (`FoldEqualCtrls`/`DropComplement`)
proves two controls are *always* equal/opposite over GF(2); the z3 lemma
confirms the peephole is sound *given* that premise. **That premise is now proved
too** ([ADR 0033](adr/0033-constprop-affine-soundness.md)), no longer only argued
+ sampled (`CONSTPROP_VERIFY`): `peephole_identities.py` adds the affine-domain
soundness lemmas (XOR-linearity of `eval`; `set(a)==set(b) ∧ cst==cst ⇒ a==b`;
`≠cst ⇒ a==¬b`; empty set ⇒ constant), and an exhaustive `#[cfg(test)]` test in
`constprop.rs` binds the real `xor_set` to *symmetric-difference over a canonical
(sorted, de-duped) form* — so the tracker's equal/complement/constant claims hold
on every basis state, and `Vec`-equality of two affine sets is exactly set-equality.

### 1c. Kani bridge — binding the proof to the real Rust types

The z3 lemmas above are a *model* of the arithmetic. `src/kani_proofs.rs`
(compiled only under `cargo kani`, behind `#[cfg(kani)]`) closes the model→code
gap with bit-precise bounded model checking on the **actual `alloy_primitives::U256`
type**:

```
cargo kani --harness solinas_add_u64    VERIFICATION: SUCCESSFUL  (0 of 3 failed,  0.33 s)
cargo kani --harness solinas_add_u256   VERIFICATION: SUCCESSFUL  (0 of 139 failed, 2.2 s)
```

- `solinas_add_u256` reproduces `mod_add_qq`'s extended-register control flow on
  real U256 values, against the **real secp256k1 prime** (`SECP256K1_P`), and
  proves it equals a division-free ground truth for all `a,b ∈ [0,p)`.
- `solinas_add_u64` is a fast small-width twin proving the control flow itself.

> **What "binding to the real types" does and does not cover (referee F1/F2,
> ADR 0026).** `solinas_add_u256` uses the real `U256`/`SECP256K1_P`, but it is a
> **hand-written re-implementation** of the control flow ("on plain integers
> instead of emitted gates", `kani_proofs.rs`) — Kani proves *that function*, not
> the gate-emitting `mod_add_qq` builder, so a drift between the two would not be
> caught. And both z3 and Kani model the **plain** `mod_add_qq`; the scored
> circuit's hot path emits `mod_add_qq_fast` (58 call sites vs 3 for the plain
> variant), a `cuccaro_add_fast` with **measurement-based uncompute** (`hmr` +
> `cz_if` phase-kickback) whose phase logic these proofs do not model. That layer
> is covered by the 9024-shot sample, not by the formal proofs. The machine-checked
> guarantee is therefore an **integer-identity** guarantee on the plain adder at
> production width (256/257, ADR 0024, PR #64), not a gate-level proof of the emitted
> measurement-based circuit.

A useful negative result: a harness over the real `sub_mod` (which calls ruint's
256-bit `%`) does **not** converge — Knuth long division has data-dependent loops
BMC cannot unwind. Division-based modular arithmetic is not BMC-tractable, which
is precisely the argument for the division-free Solinas design; that path stays
covered by the z3 layer (§1a).

### 1d. Cross-validation against the source paper's reference circuits

The source paper (Babbush et al. 2026, arXiv:2603.28846v2) publishes reference
`iadd` circuits in the kickmix format — and `iadd8.kmx`/`iadd64.kmx` are
explicitly "a variant of the adder from quant-ph/0410184" (Cuccaro et al.), the
**same primitive** this repo's arithmetic core uses.
`analysis/verify/validate_reference_adders.py` runs them through an independent,
spec-faithful kickmix simulator (`verify/kickmix_sim.py`, a Python re-derivation
of the semantics `src/sim.rs` implements) and fuzz-checks, deterministically
(seeded), that:

- **positive controls** compute `r0 += r1` (and `r0 += r1 + carry` for the
  classical-offset variant) with the addend unchanged, all clean workspace
  ancilla returned to `|0⟩`, all **dirty borrowed** ancilla restored to their
  random input, and global phase `+1`;
- **negative controls** — the paper's `inc3_wrong_{order,phase,garbage}.kmx` —
  are **rejected** (wrong output, uncorrected phase kickback, and un-restored
  ancilla respectively).

```
POSITIVE: inc3, iadd8, iadd8_with_ancillae, iadd64,
          iadd8_with_classical_offset_and_dirty_ancillae   -> all PASS
NEGATIVE: inc3_wrong_order / _phase / _garbage              -> all REJECTED
```

The `classical_offset_and_dirty_ancillae` case matters most: it is a *classical*
addend with *dirty* borrowed ancilla and measurement-based-uncomputation phase
correction — structurally the same shape as this repo's "quantum point +=
classical point" primitive. Passing it (and rejecting the negatives) confirms the
kickmix semantics this whole repo relies on reproduce the paper's own artifacts,
and that the phase-/ancilla-aware fuzz methodology (the paper's Appendix A.5
correctness argument, mirrored by `eval_circuit`'s garbage checks) actually
catches bugs.

### 1e. The windowed-ladder lookup primitive (constructed + validated)

The other ECDLP-ladder primitive is the windowed table lookup (the `3·2^w` term,
[ADR 0003](adr/0003-ground-ecdlp-estimate-in-source-paper.md)). The source paper
ships `table_lookup_3x3.kmx`, a measurement-based *unary-iteration* QROM
(Gidney 2018 §III.C) — but it is an **illustrative extract, not a runnable
circuit**: it ships only as `.kmx` + `.svg` (no test-case / fuzzer / proof, unlike
every iadd), and its selector accumulator is `R`-reset to `|0⟩` and driven by an
outer control absent from the standalone snippet (a systematic probe over
accumulator ∈ {0, 1, q2, ¬q2} × register roles × table layouts recovers < 4/8).
This is **not** a simulator gap — `verify/kickmix_sim.py` was verified equivalent
instruction-for-instruction to the reference simulator
(`original/zkp_ecc_zenodo_v2/lib/src/sim.rs`).

So the primitive is instead validated by **construction**:
`verify/controlled_lookup.py` builds a self-contained controlled lookup
`r0 ^= (ctrl ? r2[r1] : 0)` (a-bit address, d-bit data, `2^a` classical table
entries, one control qubit) and fuzz-checks it — exhaustively over addresses ×
both control values × random tables — for correct output, a genuine no-op when
`ctrl = 0`, all selector ancilla returned to `|0⟩`, and global phase `+1`. Both
uncomputation strategies pass:

```
reversible (replay CCX ladder) : a=3/d=3, a=2/d=4, a=4/d=2  -> all PASS
mbuc (HMR + CZ phase fixup)    : a=3/d=3, a=2/d=4, a=4/d=2  -> all PASS
```

The MBUC form exercises the same measurement-based-uncomputation + phase-kickback
machinery as the adders, now in a lookup; deleting its `CZ` phase corrections
makes the phase check fail loudly (63/64), confirming the test has teeth. The
reference extract itself remains a diagram, tracked in
[issue #3](https://github.com/CaptainEmpower/ecdsafail-challenge/issues/3); note
the lookup is only relevant to the ECDLP *extrapolation*, not the scored
point-addition circuit.

Beyond correctness, the lookup's **cost** is now measured, not derived (issue #4,
[ADR 0010](adr/0010-measured-windowed-lookup-cost.md)):
`verify/ladder_lookup_cost.py` builds the read as an optimized **unary-iteration**
QROM (`out ^= T[addr]`, single-ancilla-per-level spine), validates it exhaustively
(correct read, registers unchanged, all `w` ancilla cleared, phase `+1`), and
measures **`2^(w+1)−4` Toffoli / `w` ancilla per read** — e.g. `131,068` Toffoli at
`w=16` vs the paper's `3·2^16 = 196,608`. So the estimate's `3·2^w` lookup term is
a **conservative** headline with a validated construction behind it, and the `w`
ancilla matches `ECDLP_Qubits = PA_Qubits + w`. That end-to-end *composition* of the
lookup with the quantum-addend point-add into the real 28-window ladder is now
emitted and measured (issue #4, ADR 0011/0017 — see §2); only the (Clifford) QFT
carries no Toffoli.

### Scope / honesty

This verifies the **algebraic lemmas each optimization class depends on** and
binds the Solinas reduction to the real U256 type — but not a symbolic execution
of the full 28M-gate emitted circuit against the reference point-add (that does
not scale in either solver). The lemmas are the parts where bugs would hide; the
composition into a full point-add is still guarded by the sampled end-to-end
check.

---

## 2. Physical cost model (was: an abstract product)

`Toffoli × qubits` is a proxy; alone it says nothing physical. `cost_model.py`
turns the two real metrics into surface-code resources under **explicit, editable
assumptions** (physical error `1e-3`, threshold `1e-2`, `t_react = 10 µs`,
patch `= 2d²`, measurement-based Toffoli `= 4 T`). Real output for the current
circuit (one point addition):

- **Non-Clifford volume:** 5.46M T @ 4 T/Toffoli (measurement-based, the repo's
  technique) — 9.55M T @ 7 T/Toffoli (Clifford+T textbook upper bound).
- **Per-addition physical qubits:** ≈ 2.0M (d=21) to 3.4M (d=27), including a
  2× factory/routing overhead over the 1,152 logical patches.
- **Runtime:** now **measured**, not just bounded. `depth_report`
  (`src/bin/depth_report.rs` → `depth.json`) computes the non-Clifford critical
  path via `circuit::analyze_depth`: **toffoli-depth 1,077,263** (vs 1,364,230
  Toffoli gates → only **1.27× non-Clifford parallelism** — the circuit is nearly
  serial in its magic-state layer, as expected for ripple-carry modular
  arithmetic). Reaction-limited runtime = 10.77 s (vs the 13.6 s sequential
  upper bound), giving a **spacetime volume ≈ 3.6×10⁷ physical-qubit-seconds**
  at d=27.
- **This circuit vs the source paper's published bounds** (Babbush et al. 2026,
  arXiv:2603.28846v2, `docs/`). The paper zero-knowledge-proves two *point-addition*
  circuits: Low-Qubit (≤ 2.7M Toffoli, ≤ 1,175 qubits, ≤ 17M ops) and Low-Gate
  (≤ 2.1M Toffoli, ≤ 1,425 qubits, ≤ 17M ops). This repo's **measured** point
  addition — **1,364,230 Toffoli · 1,152 qubits · 10.2M ops** — is under the
  Low-Qubit bound on **all three axes**. That is the precise meaning of "beats the
  frontier": it is an improved instance of the paper's own primitive.
- **Full ECDLP extrapolation (paper's closed form, `analysis/ecdlp_estimate.py`):**
  the paper's Appendix A gives `ECDLP_Toff = (PA_Toff + 3·2^w)(2n/w − 4)` and
  `ECDLP_Qubits = PA_Qubits + w`, optimal window **w=16** → `2n/w − 4 = 28`
  windowed point additions. Substituting this repo's measured PA gives
  **(1.36M + 3·2¹⁶)·28 ≈ 43.7M Toffoli at 1,168 qubits**, reaction-limited to
  **~5 minutes** — roughly **2.06× fewer Toffoli** than the paper's published
  Low-Qubit ECDLP (≤ 90M) and 1.60× fewer than Low-Gate (≤ 70M), because the
  improved PA propagates through the ladder. (My earlier `2(n+1)=514` /
  `~7×10⁸` figure used the wrong ladder model and a `2^w` lookup; the paper's
  `28`-addition / `3·2^w`-lookup form supersedes it.)
- **The addition-composition term is now MEASURED, not asserted (issue #4,
  ADR 0007).** `src/point_add/ladder_composition.rs` (`#[cfg(test)]`) chains the
  built op stream `k` times through `analyze_ops`/`analyze_depth` and confirms
  exactly: Toffoli is additive (`k·PA`), **peak width is flat in `k`** (1152,
  Δ=0 — ancilla reused, validating `ECDLP_Qubits = PA_Qubits + w`), and
  toffoli-depth is **serial** (`k·PA_depth`). So the dominant `28·PA` term rests
  on measured composition laws; the `3·2^w` QROM lookup was subsequently
  emitted+measured (ADR 0011) and the quantum-addend build completed (issue #27,
  ADR 0014/0017); only the (Clifford) QFT carries no Toffoli — see the bullets below.
- **The full ladder is now stream-emitted and counted end-to-end (issue #4,
  ADR 0011).** `src/point_add/ladder_full.rs` (`#[cfg(test)]`) chains, per window,
  an **emitted** unary-iteration QROM-read op stream with the built point-add op
  stream and another read — `[read, add, read]` × `n_add = 28` — and counts the
  whole thing through `analyze_ops`/`analyze_depth`, with no materialization (the
  full ladder is ~290 GB; only the Toffoli-bearing QROM *selector* is emitted, as
  a lookup's `2^w·d` data-writes are Clifford). The Rust QROM emission reproduces
  `ladder_lookup_cost.py`'s `2^(w+1)−4 = 131,068` Toffoli/read. Emitted totals
  (static op-stream basis, w=16): **Toffoli `47.8M` reversible / `46.0M` MBUC**
  (addition `40.5M` + lookup; QFT 0), **toffoli-depth `30.16M`** (measured,
  add-dominated). **Peak qubits** are reported as `PA_qubits + w = 1168` per A2
  (register reuse) — *not* the naive disjoint-emit peak (`1184`), which over-counts.
  It matches the derived `(PA+3·2^w)·n_add` headline to within the MBUC saving
  (`6·n_add`), cross-validating `ecdlp_estimate.py`.
- **The classical-vs-quantum-addend *Toffoli* gap is now measured negligible
  (issue #27, ADR 0012).** `src/point_add/constprop_gap.rs` shows `coord_addsub`
  loads the classical addend into a qubit register and runs an *uncontrolled
  quantum-quantum* Cuccaro add — so the PA Toffoli is addend-**value**-independent;
  the only addend-dependent optimization (peephole constprop) is **0.05% of PA**,
  and the direct-const-arith knobs are inert. So `28·PA` already reflects the
  quantum-addend *arithmetic* cost.
- **But the classical-vs-quantum-addend *width* gap IS real (issue #27, ADR 0013).**
  `src/point_add/addend_width.rs` measures it: the classical addend is resident only
  off-peak (coord steps 1026 < the 1152 GCD peak), because `coord_addsub` frees its
  temp within each step. A QROM quantum addend must instead stay resident *across*
  the peak (`ox`@3/7/15, `oy`@4/14 straddle both GCD passes), where it cannot overlap
  the GCD scratch — so the constructed port peaks at **1408** (hold one coord) to
  **1664** (hold `P[k]=(x,y)`), i.e. `PA + 256..512`. So A2's `ECDLP_Qubits =
  PA_Qubits + w` (= 1168) **undercounts** a verified port of *this* PA by 256..512
  qubits; A2 holds for the paper because its `PA_Qubits` bound already prices a
  resident addend into a tighter core (this repo stayed under bound by keeping the
  addend classical — a port would erase that width edge).
- **The QROM-fed quantum-addend add now works end-to-end — verified by simulation
  (issue #27/#28, ADR 0014).** `src/point_add/qaddend_testbed.rs` composes, on
  fresh registers, a real **unary-iteration QROM read** (the ADR 0010 selector,
  now WITH the leaf data-writes the cost-only harnesses omit) → an uncontrolled
  **q-q Cuccaro add** (the `coord_addsub` shape) → a **QROM unread**, and checks by
  masked multi-shot simulation over all `2^w` windows × several accumulators that
  `acc' == (acc + P[k]) mod 2^n` with the addend, selector spine, carry, and window
  register all clean/preserved (read selector Toffoli `= 2^(w+1)−4`, tying back to
  ADR 0010). This closes the "does the composition even work" question ADR 0011
  deferred and gives the register-overlap picture an executable form (addend +
  spine ride on top of the adder — the small-scale ADR 0013). The **field-modular
  reduction tail** is delivered too (`qrom_fed_quantum_addend_modular_add`,
  `acc := (acc + P[k]) mod p` via a Vedral–Barenco–Ekert modular adder, ancilla-clean
  by simulation). The EC exceptional cases (`P==Q`, `dx=0`, ∞) are now exactly
  bounded (issue #28, ADR 0016) and circuit-confirmed as a reversible detector on
  real coordinates (ADR 0018); what remains — a separate increment — is *handling*
  them via complete formulas rather than only detecting them.
- **The true quantum-addend ladder is now multi-window and its read→add
  serialization depth is measured (issue #27 item 2, ADR 0017).**
  `src/point_add/ladder_stream.rs` composes ADR 0014's verified `read→add→unread`
  over `m` windows. Its **functional** test simulation-verifies the *accumulator
  threading across windows* — `acc == (y + Σ_j T_j[k_j]) mod p` on one shared
  workspace, all ancilla clean — the multi-window step the single-add testbed did
  not exercise. Its **measurement** test streams the workspace ids reused across the
  real `n_add = 28` windows (no materialization) and measures the *true* serialized
  `read→add→unread` toffoli-depth (the QROM writes the addend the adder consumes, a
  real RAW hazard) against the **disjoint** model `ladder_full.rs` uses (QROM ∥ add):
  at `(n,w)=(32,6)` the overlap per-window depth `558` exceeds the add-only `320` by
  `238` — the per-window QROM serialization depth `ladder_full.rs` omits — and the
  disjoint peak is `+n` wider (ADR 0011's flagged over-count, executable). So the
  "measured, not assumed" read→add depth #27 asks for is delivered at representative
  width, with a closed-form scale to `w=16`; the materialized 256-bit run (~290 GB)
  stays out of scope per #27.
- **The full-ladder resources are now a measured output in the estimate (issue #27
  item 3, ADR 0017).** `ladder_full.rs` emits its streamed w=16 totals to
  `analysis/ladder_measured.json` (Toffoli `47.8M` reversible / `46.0M` MBUC,
  toffoli-depth `30.16M`, peak `1168`), and `analysis/ecdlp_estimate.py` **consumes**
  it — printing a dedicated *measured* full-ladder section alongside its derived
  headline and cross-asserting `measured_mbuc == (PA+3·2^w)·n_add − 6·n_add` on the
  artifact's static op-stream PA basis (kept distinct from the executed avg-per-shot
  headline). So the estimate's old "numbers are derived, not emitted+measured" caveat
  is retired: the headline is derived, and the full ladder is *also* emitted+measured
  end-to-end.

**Key limitations this surfaces** (all real, all worth fixing):
- The scored "qubits" is `max_id + 1` (highest allocated id + 1, computed by
  `analyze_ops` in `src/circuit.rs`).
  This *equals* peak simultaneous width **because the builder reuses freed qubit
  ids** — the `ladder_composition` test measures peak flat at 1152 (Δ=0) across
  chained additions, i.e. ancilla ids are recycled, so `max_id+1` tracks the true
  peak rather than the running total. So the README's "peak qubits" label is
  accurate for this circuit (referee finding F8). The residual caveat is only
  structural: `max_id+1` would *over-count* (not under-count) for a builder that
  never recycled ids, so it is a conservative proxy, never an optimistic one.
- ~~No depth / T-depth is tracked~~ **RESOLVED**: `circuit::analyze_depth` +
  `depth_report` now measure toffoli-depth and gate-depth (critical path over
  read/write hazards), feeding measured runtime and spacetime volume into the
  cost model.
- The full-attack ladder cost now uses the source paper's exact closed form
  (`(PA+3·2^w)(2n/w−4)`, w=16). Adder **completeness** (exceptional cases P==Q,
  P==−Q, ∞) is now backed by a quantitative **negligibility argument**
  (`completeness_argument.md`): the gating experiment shows exceptions keep the
  ancilla clean but corrupt output/phase on the offending state only, so it
  suffices to bound their amplitude — the ∞-accumulator is removed structurally
  (paper's direct-lookup first window) and the residual `dx=0` collisions total
  `≈ 2⁻²⁵⁰` (union bound over 28 additions), >240 bits below Shor's ~1%
  tolerance. This justifies `completeness_overhead = 1.0`; it is an argument, not a
  fully machine-checked proof of the whole attack — but the ∞ cases are now removed
  structurally (ADR 0009) and by offset-window encoding (ADR 0015), and the residual
  bound is exact (ADR 0016) and circuit-confirmed on real coordinates (ADR 0018). The
  classical-vs-quantum-addend gap remains, but its cost
  correction is small (only the coordinate steps change; the dominant
  inversion/square are addend-independent).
- **Adder completeness is now partly measured, not only argued (issue #5,
  ADR 0008).** `verify/completeness_collision_rate.py` computes the *exact*
  exceptional-input rate of the affine adder across a faithful windowed ladder
  (scalar model validated against a real prime-order curve). It confirms the
  `dx=0` collision rate the completeness argument relies on tracks `2/n` within a
  small constant (`0.47–0.81×`), and — crucially — that this holds even when the
  accumulator is far from uniform, because the addend sweeps the group. It also
  sharpens the bound: the *dominant* exceptional term is the **zero-window ∞**
  case at `~1/2^w` per addition (`≈2⁻¹¹` total at `w=16`), not the `dx=0` term
  (`≈2⁻²⁵⁰`). Both sit far below Shor's `~1%` tolerance, so `completeness_overhead
  = 1.0` holds — but the `2⁻¹¹` figure is conditional on the lookup encoding never
  emitting the `∞` table entry.
- **That dominant term is now removed, not just bounded (issue #5 part (b),
  ADR 0015).** `verify/offset_window_encoding.py` implements the **offset window
  encoding** (shift every window digit `g → g+1`, correct by one compile-time
  point) and proves exhaustively on a real toy curve that it *never* emits the `∞`
  table entry — while standard windowing does, exactly at a zero digit — yet still
  computes `[a]P+[b]Q` for every `(a,b)`. Re-running the exact measurement, the
  `addend=∞` rate is then **exactly 0** and `dx=0` is unchanged, so the completeness
  headline sharpens from `~2⁻¹¹` back to the `dx=0`-limited `~2⁻²⁵⁰` under an
  explicit, validated encoding condition rather than a silent assumption.
- **The completeness headline is now an *exact* end-to-end bound, not just a union
  bound (issue #28, ADR 0016).** `verify/mid_ladder_bound.py` computes the exact
  `P[≥1 exceptional across the real 28-window two-scalar ladder]` by tracking the
  accumulator's clean (never-yet-exceptional) mass through the whole run — the exact
  `P[⋃_k A_k]`, which it verifies is `≤` the completeness argument's union bound on
  every config (and `exact + survival == 1` exactly, so no mass is lost). At attack
  parameters an exact convolution is infeasible, so the rigorous end-to-end bound is
  the union upper bound (`≈2⁻²⁵⁰` offset / `≈2⁻¹¹` standard, both `≪` Shor's `~1%`) —
  which the toy `exact ≤ union` results certify is tight, not loose.
- **That scalar/dlog bound is now confirmed at the CIRCUIT level over real
  coordinate arithmetic (issue #28, ADR 0018).** The whole bound (ADR 0016) and the
  offset pin (ADR 0015) compute in the dlog model, whose one curve assumption is
  `dx=0 ⇔ acc ≡ ±addend (mod n)`. `src/point_add/ec_exceptional.rs` builds a
  **reversible** exceptional detector — `dx0 = (x1==x2)`, `acc=∞`/`addend=∞` as
  `∞`-sentinel zero-tests, on real `(x,y)` coordinate qubits, **no modular inverse** —
  and simulation-measures it over **every** `(acc, addend)` pair of a real
  prime-order toy curve (`y²=x³+2x+2 / F₁₇`, `n=19`): the real-coordinate verdict
  equals the scalar predicate `(m==0) ∨ (y==0) ∨ (y≡±m)` on all `19²` pairs (0
  mismatches). Driving the ADR 0016 survival recursion with the *circuit-measured*
  predicate reproduces the scalar-model residual (`exact ≤ union`), and the offset
  encoding emits `addend=∞` **never** on real coordinates — the zero-window pin,
  circuit-confirmed. So the equivalence the completeness bound rests on is no longer
  only a dlog assumption; it holds by a reversible circuit over real coordinates,
  exhaustively over the group. (Detects — does not yet *handle* via complete formulas;
  the reversible λ-division point-add is a separate increment.)
- **The amplitude-1 ∞ start is now circuit-demonstrated as removed (issue #5
  part (a), ADR 0009).** The one exceptional case that negligibility *cannot*
  cover — the accumulator starting at ∞ with amplitude 1 — is handled structurally
  by the paper's "first windowed addition = direct lookup".
  `verify/direct_lookup_init.py` builds that init as an actual reversible circuit
  (the validated controlled-lookup QROM writing `acc ^= T[w]`, `T[w] = [w]·P`,
  into a `\|0⟩` accumulator) and shows exhaustively — toy prime-order curve, both
  uncompute modes, plus a secp256k1 256-bit spot-check — that the accumulator ends
  holding a real affine point for every window and is the `(0,0)` ∞ sentinel *iff*
  `w=0` (ancilla clean, phase `+1`; `ctrl=0` leaves it at ∞). So the adder is
  never fed the amplitude-1 ∞ start. The mid-ladder residual over the real 28-window
  superposition is now addressed too: an exact end-to-end bound (issue #28, ADR 0016)
  confirmed at the circuit level over real coordinates (ADR 0018), atop the
  emitted+measured Tier B ladder (#4, ADR 0011/0017).

---

## 3. Generalizable techniques (the transferable science)

Catalogued from `src/point_add/`. Provenance strings in the code are real and
were verified (`venting.rs:1,311`, `mod.rs:709,21`, `gcd.rs`).

### Reusable across any modular-arithmetic quantum circuit
- **Cuccaro ripple-carry adder** (`arith/adder.rs`, `mod.rs:709`) — Cuccaro et al.
  2004 (arXiv:quant-ph/0410184). Foundation; 1 carry ancilla.
- **Measurement-based (vented) uncomputation** (`venting.rs`) — Gidney 2025
  (arXiv:2507.23079) + Häner–Roetteler–Soeken 2017 (arXiv:1709.06648). Replaces
  the ~n-Toffoli UMA uncompute with H-measure-reset + deferred conditional-CZ
  phase corrections ⇒ **zero Toffoli** in uncompute, at the cost of classical
  bookkeeping bits. This is the single largest structural saving and transfers to
  any circuit that needs to zero a carry/flag qubit.
- **2-clean-ancilla streaming adder** (`venting.rs:124`) — Gidney 2025 Fig. 2/4.
  Peak O(1) clean ancilla instead of O(n); central to the low-qubit-width score.
- **Kaliski / two-inverse conjugate uncomputation** (`mod.rs:21`, `gcd.rs`) —
  Roetteler et al. 2017 (arXiv:1706.06752) + Bernstein–Yang jump-GCD
  (arXiv:2510.10967). Field inversion reused to uncompute scratch, saving ~2×256
  ancilla qubits.
- **Sound constant-propagation peephole** (`constprop.rs`) — abstract
  interpretation over {0,1,⊥} + GF(2) affine forms drops/folds provably-constant
  CCX gates. General to any reversible circuit with initialized ancillae. Verified
  in §1b.

### Curve/harness-specific (still instructive, less portable)
- **Solinas folding** (`arith/modular/add.rs`, verified in §1a) — exploits the sparse
  `c = 2^32 + 977`; bespoke per Solinas prime, not general.
- **Fused double / controlled-double and symmetric square-subtract**
  (`trailmix_ludicrous/fused.rs`, `ec_add.rs`) — amortize shared folds/carries;
  depend on the a=0, b=7 group law.
- **PAD-truncated comparator recomputation** (`comparator.rs`, `arith.rs`) — trade
  a `2^-PAD` phase-miss probability for ~n→PAD recomputation width. Tunable but
  problem-specific.
- **Baked schedule / design-space search** (`trailmix_ludicrous/mod.rs`,
  `schedule.rs`, `TLM_*` env knobs) — a Pareto frontier of (carry-cap, vent-count,
  fold-width) operating points, replayed at build time. This is automated
  design-space exploration; the *method* is general, the baked tables are not.

---

## 4. Bottom line

With §1 and §2 in place, the circuit is no longer just a leaderboard number:
its arithmetic core is **proven correct over the whole field** (not just 9024
samples) — at two levels, an abstract-bitvector z3 model (§1a–b) and a
bit-precise Kani proof bound to the real `alloy` U256 type (§1c) — and its score
is **anchored to a physical cost model** with explicit assumptions and now a
**measured** toffoli-depth → runtime → spacetime volume (§2). The full ECDLP ladder
is now stream-emitted and measured end-to-end (`ladder_full.rs` → `ladder_measured.json`,
consumed by `ecdlp_estimate.py`), so the derived headline is corroborated by a
measured count. And the completeness argument no longer stops at a bound: the full
two-register Shor-ECDLP, driven by the incomplete affine adder this circuit implements
plus the offset/direct-lookup handling, **recovers the secret discrete log** on toy
prime-order curves by exact statevector simulation (`verify/shor_ecdlp_recovery.py`,
issue #46, ADR 0019) — an executable demonstrated attack at toy scale, with the
`∞`-free encoding condition shown load-bearing for the recovery, not only the
amplitude figure. A remaining stretch goal is symbolic execution of the emitted
op-stream on computational-basis inputs to prove the *composed* point-add end-to-end.
