# ADR 0017 — True quantum-addend windowed ladder: multi-window, register-overlapped, depth-honest (Tier B, issue #27 item 2)

**Status:** Accepted — implements issue #27's deliverable 2 (compose the
quantum-addend read→add→unread into the real windowed ladder and stream-measure
end-to-end); built in `src/point_add/ladder_stream.rs`.
**Date:** 2026-07-03

## Context

Issue #27 asks for two things beyond the single-add testbed of
[ADR 0014](0014-quantum-addend-testbed.md):

1. a functionally-correct QROM-fed quantum addend — **done** (ADR 0014 v1/v2:
   `qrom_fed_quantum_addend_add`, `qrom_fed_quantum_addend_modular_add`), and
2. **compose `QROM read → add → unread` into the real 28-window ladder and
   stream-measure end-to-end Toffoli / peak-qubits / depth with the real register
   overlap (no materialization).**

Item 2 is the piece the two existing cost harnesses each leave open:

- [ADR 0011](0011-streamed-full-ladder.md) / `ladder_full.rs` streams
  `[read_selector, PA, read_selector] × 28`, but emits the QROM selector on
  **disjoint ids** and uses the **classical-addend** PA. So (a) the addition never
  consumes the loaded addend — there is no read→add data dependency — and (b) the
  QROM therefore composes *in parallel* with the additions, making the reported
  toffoli-depth **add-only**. `ladder_full.rs` flags this itself: "Emitting the
  lookup on disjoint ids UNDER-counts the serial depth … the true read→add
  dependency … should be measured, not assumed."
- [ADR 0014](0014-quantum-addend-testbed.md) / `qaddend_testbed.rs` builds the
  **true** read→add→unread where the addend register the QROM writes is the one the
  adder consumes — but only for a **single** window, so it never exercises the
  accumulator threading *across* windows nor the streamed multi-window metrics.

## Decision

Add `ladder_stream.rs`: a **multi-window** ladder over the *true* quantum-addend
read→add→unread (the ADR 0014 shape — unary-iteration QROM read *with data-writes*
→ uncontrolled quantum-quantum Cuccaro modular add that consumes the addend → QROM
unread), reusing ADR 0014's verified `qrom_read` / `mod_add` fragments. Two faces:

1. **Functional (fast, default suite) — `streamed_windowed_ladder_accumulates_mod_p`.**
   A real multi-window ladder: a distinct `w`-bit scalar window per step
   (`m·w`-qubit scalar register, the honest ladder shape) feeding **one shared
   arithmetic workspace** (addend / accumulator / selector spine / carry / modular
   ancilla / `p`-scratch), reused across all `m` windows with per-window tables
   `T_j`. Verified by masked multi-shot simulation that the accumulator threads
   correctly window-to-window: `acc == (y + Σ_j T_j[k_j]) mod p` over all shots,
   with the addend, spine, carry, and every modular ancilla returned to |0>. This
   is the new correctness result — ADR 0014 only proved a *single* add.

2. **Measurement (`#[ignore]`, heavy) — `streamed_windowed_ladder_measured`.**
   At a representative `(n, w)`, reuse the workspace ids across `m` windows and
   **stream** the op emission through a per-window closure via `flat_map` (only one
   window's ops are ever materialized — a full 256-bit ladder is ~290 GB, out of
   scope per #27). Measure with `analyze_ops` / `analyze_depth`:
   - **Toffoli** total `= m · per_window` (each read `2^(w+1)−4`, asserted);
   - **Peak qubits** — the workspace with the addend + spine held **resident**
     across every window (the executable analogue of ADR 0013's full-width
     +256..512);
   - **True toffoli-depth** — because the QROM read writes the same `addend` ids the
     adder reads (a real RAW hazard), the read→add→unread **serializes**; the
     measured per-window depth is `read_depth + add_depth + unread_depth`. Built
     alongside is the **disjoint** variant (QROM writing a *separate* scratch, à la
     `ladder_full.rs`), whose per-window depth is `max(read, add) = add` — the
     add-only undercount. The measured delta between them **is** the read→add
     serialization depth `ladder_full.rs` omitted, now a number rather than a
     caveat. The disjoint variant's wider peak (+`n` for the separate addend
     scratch) reproduces ADR 0011's flagged disjoint-emit over-count.

   Report both the measured figures and a **closed-form scale-up** to the real
   `w=16, n_add=28` headline, cross-checking `ecdlp_estimate.py` — honestly labeled
   as small-width-executable + closed-form, not a materialized 256-bit run.

## Consequences

- **Issue #27 item 2 is delivered.** The read→add data dependency and register
  overlap that ADRs 0011/0014 deferred are now *measured* on a streamed multi-window
  ladder: the accumulator threading is simulation-verified, the resident-addend peak
  is measured, and the read→add serialization depth `ladder_full.rs` under-counted
  is quantified (overlap depth − disjoint depth).
- **The depth caveat becomes a measurement.** `ladder_full.rs` reports an add-only
  toffoli-depth and flags the QROM as omitted; this harness measures the true serial
  read→add→unread depth and the exact QROM contribution, at representative width.
- **Scope (honest).** Still the *scalar/arithmetic* model (as ADR 0014): the adder
  is the width-parametric quantum-quantum modular add, not a 256-bit coordinate
  point-add, and the measurement runs at a representative `(n, w)` with a closed-form
  scale to `w=16`. The full-width materialized ladder (~290 GB) is explicitly out of
  scope per #27. Issue #28's EC exceptional cases (`P==Q`, `dx=0`, ∞) still need the
  group law on top of this substrate and remain a separate increment.
- **Item 3 (optional).** Emitting the streamed totals to a JSON artifact for
  `ecdlp_estimate.py` to consume alongside its closed-form headline is a small
  follow-up, deferred to keep this increment focused.
- Consistent with [ADR 0001](0001-analysis-layer-isolated-from-score.md): the
  harness is `#[cfg(test)]`, never compiled into `build_circuit`; the scored circuit
  is byte-identical (`ops.bin` SHA unchanged).
