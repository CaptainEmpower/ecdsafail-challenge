# Architecture Decision Records

Records of the significant decisions behind the `analysis/` scientific-rigor
layer. Each ADR is immutable once **Accepted**; a later decision that changes
course supersedes it with a new record rather than editing history.

Format: Status · Context · Decision · Consequences (lightweight MADR).

| ADR | Title | Status |
|---|---|---|
| [0001](0001-analysis-layer-isolated-from-score.md) | Analysis layer is isolated from the scored circuit | Accepted |
| [0002](0002-derived-ecdlp-ladder-factor.md) | Derive the full-ECDLP ladder factor instead of hand-picking it | Accepted (ladder model superseded by 0003) |
| [0003](0003-ground-ecdlp-estimate-in-source-paper.md) | Ground the ECDLP estimate in the source paper's closed form | Accepted |
| [0004](0004-cross-validate-against-reference-circuits.md) | Cross-validate against the source paper's reference circuits | Accepted |
| [0005](0005-validate-lookup-by-construction.md) | Validate the ladder lookup primitive by construction | Accepted |
| [0006](0006-adder-completeness-approach.md) | Approach to adder completeness (cost estimate → verified attack) | Accepted (Path A viable per gating experiment) |
| [0007](0007-tier-b-measured-ladder.md) | Tier B: measuring the full-ECDLP ladder | Accepted (first increment; QROM lookup now measured, ADR 0010/0011; QFT semiclassical/Clifford) |
| [0008](0008-empirical-completeness-collision-rate.md) | Empirically validate (and sharpen) the completeness collision rate | Accepted (equidistribution validated; zero-window ∞ term dominant) |
| [0009](0009-direct-lookup-init.md) | Circuit-demonstrate the ∞-start removal (direct-lookup first window) | Accepted (amplitude-1 ∞ start removed; mid-ladder residual now bounded exactly in ADR 0016 and circuit-confirmed in ADR 0018) |
| [0010](0010-measured-windowed-lookup-cost.md) | Measure the windowed-lookup (QROM) cost (Tier B, issue #4) | Accepted (lookup term grounded; end-to-end ladder now emitted+measured, ADR 0011/0017) |
| [0011](0011-streamed-full-ladder.md) | Stream-emit and measure the full ECDLP ladder (Tier B, issue #4) | Accepted (ladder emitted+measured; quantum-addend PA now sim-verified, ADR 0014/0017) |
| [0012](0012-classical-vs-quantum-addend-gap.md) | The classical-vs-quantum-addend gap is negligible (Tier B, issue #27) | Accepted (Toffoli gap ≤0.05% measured; width/register-overlap remains) |
| [0013](0013-quantum-addend-width-gap.md) | The quantum-addend WIDTH gap is real: A2's `+w` undercounts this PA (Tier B, issue #27) | Accepted (measured port needs PA+256..512+w; functional QROM-fed add remains) |
| [0014](0014-quantum-addend-testbed.md) | Quantum-addend point-add testbed: a QROM-fed add, verified by simulation (Tier B, issue #27/#28) | Accepted (QROM read→q-q modular add→unread sim-verified; EC-exceptional detection now circuit-confirmed, ADR 0018) |
| [0015](0015-offset-window-encoding.md) | Offset window encoding removes the zero-window ∞ exceptional term | Accepted (dominant ∞ term removed structurally; bound sharpened to ~2⁻²⁵⁰) |
| [0016](0016-exact-mid-ladder-bound.md) | Exact end-to-end bound on the mid-ladder exceptional amplitude | Accepted (exact <= union, <<1% at attack scale; circuit-level real-coordinate confirmation done, ADR 0018) |
| [0017](0017-true-quantum-addend-ladder.md) | True quantum-addend windowed ladder: multi-window, register-overlapped, depth-honest (Tier B, issue #27 item 2) | Accepted (multi-window accumulation sim-verified; read→add serialization depth measured vs ladder_full's disjoint model) |
| [0018](0018-circuit-level-exceptional-detection.md) | Circuit-level EC exceptional detection over real coordinates (completeness, issue #28/#5) | Accepted (reversible dx=0/∞ detector matches the scalar/dlog model on the whole toy group; offset ∞-pin circuit-confirmed) |
| [0019](0019-end-to-end-ecdlp-recovery.md) | End-to-end Shor-ECDLP discrete-log recovery on toy curves (demonstrated attack, issue #46) | Accepted (full pipeline recovers the secret m with the incomplete adder + offset/direct-lookup; complete P_success=(n-1)/n, offset recovers m, standard degraded — the executable complement to ADR 0016/0018) |
| [0020](0020-reversible-toy-modular-inverse.md) | Reversible toy-width modular inverse (Path B prerequisite, issue #48) | Accepted (built in `toy_field.rs` — reversible `mod_mul` + Fermat `mod_inv`, verified exhaustively over F_p; `emit_mod_mul`/`emit_mod_inv` ready for 0021) |
| [0021](0021-reversible-lambda-division-point-add.md) | Reversible λ-division affine point-add with exceptional handling (Path B, toy scale, issue #48) | Accepted (built in `toy_pointadd.rs` — complete affine adder handling doubling/P=−Q/∞, verified over every (P,Q) pair of order-19/29/41 toy curves; the "handle not just detect" increment) |
| [0022](0022-gate-level-toy-shor-capstone.md) | Gate-level QFT toy Shor-ECDLP capstone: unifying the gate-level pieces (issue #55) | Accepted (built in `toy_shor_qft.py` + the `gate_level_ladder` test — real gate-level QFT + complete point-add oracle applied as a permutation; recovers the secret m on order-7/11 toy curves; the "fully gate-level" stretch of 0021 §4) |
| [0023](0023-external-referee-review.md) | External referee review and remediation index (issue #62) | Accepted (independent review reproduced the score + suite; findings F1–F8 on framing/citations split into ADR 0024/0025/0026; `paper/REVIEW.md`) |
