//! F2 (referee review `paper/REVIEW.md`, ADR 0026 note, ROADMAP) — a Kani
//! (bit-precise BMC) proof of the emitted `_fast` adder that is **bound to the
//! real emitter and the real simulator**, not a hand-written twin.
//!
//! The existing Kani harnesses (`src/kani_proofs.rs`) prove `solinas_add` on plain
//! integers — a faithful *copy* of `mod_add_qq`'s control flow, but a copy. F2's
//! sharp point is exactly that: if the copy and the gate-emitting builder drift, a
//! proof over the copy stays green. ADR 0027 closed the *phase* half of that gap in
//! z3 over the emitted op-stream; this closes it on the Rust side, with Kani driving
//! the **actual `B` builder** (`cuccaro_add_fast`, `arith/adder.rs`) and the
//! **actual `Simulator`** (`src/sim.rs`) — the same code the scorer runs.
//!
//! [`drive_and_check`] emits `cuccaro_add_fast` at a small width, runs it on the
//! real simulator over shot 0 with the register values free, and asserts the full
//! contract:
//!   1. FUNCTIONAL     `acc' == (a + acc) mod 2^n`
//!   2. A-PRESERVED    `a` returns to its input
//!   3. ANCILLA-CLEAN  `c_in` and every carry ancilla back to |0>
//!   4. PHASE-CLEAN    net phase bit 0 == 0 — HMR kickback `carry·m` cancelled by `cz_if`
//!
//! The **measurement outcomes are free**: the simulator's 64 shots are fully
//! independent (every op is bitwise-parallel across lanes), so populating only
//! shot 0 and asserting on bit 0 models one shot with a free outcome. The
//! [`XofReader`] feeding the HMR randomness returns a free bit per read under Kani,
//! so the proof holds for **all inputs and all measurement outcomes** — the same ∀
//! guarantee as the z3 proof (ADR 0027), now on the real Rust types and the real
//! emitter/simulator.
//!
//! One [`drive_and_check`] body serves both the `#[kani::proof]` harnesses (symbolic
//! inputs + symbolic outcomes, widths 2/3) and an exhaustive `#[cfg(test)]` shadow
//! (concrete, every input and every outcome at widths 2/3/4). The shadow runs in the
//! normal `cargo test` job, so the harness is exercised — and guarded against
//! bit-rot — even where `cargo kani` is unavailable.
//!
//! `#[cfg(any(test, kani))]` only; never compiled into `build_circuit`, so the
//! scored circuit and `ops.bin` are untouched (ADR 0001).

use super::{cuccaro_add_fast, B};
use crate::circuit::{QubitId, QubitOrBit};
use crate::sim::Simulator;
use ruint::aliases::U256;
use sha3::digest::XofReader;

/// Emit `cuccaro_add_fast` at width `n` via the real builder, simulate it over
/// shot 0 with register inputs `a_val`/`acc_val` (masked to `n` bits) and the
/// HMR randomness drawn from `reader`, and assert the four-part correctness
/// contract. Panics (fails the proof / the test) on any violation.
fn drive_and_check<R: XofReader>(n: usize, a_val: u64, acc_val: u64, reader: &mut R) {
    assert!((1..=64).contains(&n));
    let mask = if n == 64 { u64::MAX } else { (1u64 << n) - 1 };
    let a_val = a_val & mask;
    let acc_val = acc_val & mask;

    // Emit the real gate stream for `acc += a (mod 2^n)`.
    let mut b = B::new_for_test();
    let a = b.alloc_qubits(n);
    let acc = b.alloc_qubits(n);
    let c_in = b.alloc_qubit();
    cuccaro_add_fast(&mut b, &a, &acc, c_in);
    let ops = b.take_ops();
    let num_qubits = b.next_qubit as usize;
    let num_bits = b.next_bit as usize;

    // Run the real simulator on shot 0; c_in and the carry ancillae default to |0>.
    let mut sim = Simulator::new(num_qubits, num_bits, reader);
    let a_reg: Vec<QubitOrBit> = a.iter().map(|&q| QubitOrBit::Qubit(q)).collect();
    let acc_reg: Vec<QubitOrBit> = acc.iter().map(|&q| QubitOrBit::Qubit(q)).collect();
    sim.set_register(&a_reg, U256::from(a_val), 0);
    sim.set_register(&acc_reg, U256::from(acc_val), 0);
    sim.apply_iter(ops.iter());

    // (1) FUNCTIONAL: acc' == (a + acc) mod 2^n.
    let expect = (a_val.wrapping_add(acc_val)) & mask;
    assert!(
        sim.get_register(&acc_reg, 0) == U256::from(expect),
        "functional: acc' == (a + acc) mod 2^n"
    );
    // (2) A-PRESERVED.
    assert!(
        sim.get_register(&a_reg, 0) == U256::from(a_val),
        "a preserved"
    );
    // (3) ANCILLA-CLEAN: every qubit that is not part of a/acc is back to |0>
    //     on shot 0 (this is c_in and the n-1 carry ancillae).
    for id in 0..num_qubits {
        let qid = QubitId(id as u64);
        if a.contains(&qid) || acc.contains(&qid) {
            continue;
        }
        assert!(
            sim.qubit(qid) & 1 == 0,
            "c_in / carry ancilla returns to |0>"
        );
    }
    // (4) PHASE-CLEAN: net global phase on shot 0 is 0 — the HMR kickback is
    //     exactly cancelled by the cz_if correction, for this input and outcome.
    assert!(
        sim.phase & 1 == 0,
        "net phase 0 (HMR kickback cancelled by cz_if)"
    );
}

// ── Kani harnesses: symbolic inputs + symbolic measurement outcomes ──────────
#[cfg(kani)]
mod proofs {
    use super::*;

    /// Feeds the simulator's HMR reads a **free** measurement bit per read. Only
    /// bit 0 is populated because only shot 0 is populated and the lanes are
    /// independent; leaving the other 63 bits 0 keeps the symbolic state minimal
    /// while still ranging over every outcome relevant to shot 0.
    struct KaniXof;

    impl XofReader for KaniXof {
        fn read(&mut self, buffer: &mut [u8]) {
            for byte in buffer.iter_mut() {
                *byte = 0;
            }
            if !buffer.is_empty() {
                buffer[0] = if kani::any() { 1 } else { 0 };
            }
        }
    }

    #[kani::proof]
    fn mbuc_fast_adder_width2() {
        let a_val: u64 = kani::any();
        let acc_val: u64 = kani::any();
        let mut xof = KaniXof;
        drive_and_check(2, a_val, acc_val, &mut xof);
    }

    #[kani::proof]
    fn mbuc_fast_adder_width3() {
        let a_val: u64 = kani::any();
        let acc_val: u64 = kani::any();
        let mut xof = KaniXof;
        drive_and_check(3, a_val, acc_val, &mut xof);
    }
}

// ── Shadow: exhaustive over all inputs and all outcomes, in `cargo test` ─────
#[cfg(test)]
mod shadow {
    use super::*;

    /// Deterministic outcome source: returns the `idx`-th bit of `bits` as the
    /// measurement result of the `idx`-th HMR read (byte 0 = the bit, rest 0), so
    /// the caller can enumerate every measurement-outcome combination.
    struct FixedXof {
        bits: u64,
        idx: usize,
    }

    impl XofReader for FixedXof {
        fn read(&mut self, buffer: &mut [u8]) {
            let bit = ((self.bits >> self.idx) & 1) as u8;
            self.idx += 1;
            for byte in buffer.iter_mut() {
                *byte = 0;
            }
            if !buffer.is_empty() {
                buffer[0] = bit;
            }
        }
    }

    /// Exhaustive over every input pair AND every measurement-outcome combination
    /// at widths 2/3/4, driving the real emitter + simulator. This is the concrete
    /// twin of the Kani proof: what Kani discharges symbolically at width 2/3, this
    /// checks by enumeration (incl. width 4), so the shared `drive_and_check` body
    /// is covered by the normal `cargo test` job regardless of `cargo kani`.
    #[test]
    fn mbuc_fast_adder_exhaustive_shadow() {
        for &n in &[2usize, 3, 4] {
            let n_hmr = n - 1; // one HMR carry-clear per non-LSB bit
            let span = 1u64 << n;
            for a_val in 0..span {
                for acc_val in 0..span {
                    for outcomes in 0..(1u64 << n_hmr) {
                        let mut xof = FixedXof {
                            bits: outcomes,
                            idx: 0,
                        };
                        drive_and_check(n, a_val, acc_val, &mut xof);
                    }
                }
            }
        }
    }
}
