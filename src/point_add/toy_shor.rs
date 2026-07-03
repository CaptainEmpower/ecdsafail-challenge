//! Capstone provenance (issue #55, ADR 0022) — the **gate-level ladder** that grounds
//! the toy-Shor oracle. `[a]P + [b]Q` is computed by chaining the ADR 0021 complete
//! point-add (`emit_point_add`) as a repeated-addition ladder on the simulator, and the
//! accumulator read-back is asserted to equal the reference group law on a real
//! prime-order toy curve — the gate-level arithmetic oracle that the gate-level-QFT run
//! `analysis/verify/toy_shor_qft.py` applies as a permutation to recover the secret `m`.
//!
//! Child module of [`super`] (`toy_pointadd`): it reuses that module's `#[cfg(test)]`
//! point-add internals directly (`emit_point_add`, `load_const`, `read_reg`, the
//! classical `Pt`/`ToyCurve`/`ec_*` reference) rather than re-exposing them. Lives at
//! `src/point_add/toy_shor.rs` per the issue #55 compliance checklist; `#[cfg(test)]`
//! only, never compiled into the scored circuit (`ops.bin` unchanged).

use super::super::toy_field::width_for;
use super::{coords, ec_add, ec_mul, emit_point_add, load_const, read_reg, Pt, ToyCurve};
use crate::circuit::analyze_ops;
use crate::point_add::B;
use crate::sim::Simulator;

/// Compute `[a]P + [b]Q` by chaining the **gate-level** complete point-add
/// (`emit_point_add`) as a repeated-addition ladder, then read the accumulator out
/// of the simulator — the gate-level arithmetic oracle the toy-Shor capstone relies
/// on. `acc` starts at the `∞` sentinel `(0,0)`; each step adds a fixed classical
/// point loaded via `load_const`. The ladder naturally exercises the exceptional
/// branches (∞ start, doublings at `[k]P + P`, and `→ ∞` at `[-1]P + P`).
fn ladder_eval(a: u64, b: u64, gen: Pt, q: Pt, curve_a: u64, p: u64, n: usize) -> (u64, u64) {
    let mut circ = B::new_for_test();
    let mut accx = circ.alloc_qubits(n); // acc = ∞ = (0,0): already |0>
    let mut accy = circ.alloc_qubits(n);
    let steps: Vec<(u64, u64)> = std::iter::repeat_n(coords(gen), a as usize)
        .chain(std::iter::repeat_n(coords(q), b as usize))
        .collect();
    for (tx_v, ty_v) in steps {
        let tx = circ.alloc_qubits(n);
        let ty = circ.alloc_qubits(n);
        load_const(&mut circ, &tx, tx_v);
        load_const(&mut circ, &ty, ty_v);
        let ox = circ.alloc_qubits(n);
        let oy = circ.alloc_qubits(n);
        emit_point_add(&mut circ, &accx, &accy, &tx, &ty, &ox, &oy, curve_a, p, n);
        accx = ox;
        accy = oy;
    }
    let ops = circ.take_ops();
    let (peak, nbits, _r, _regs) = analyze_ops(ops.iter());
    // an empty ladder (a=b=0) emits no ops (acc stays ∞=(0,0)); size the sim to at
    // least the accumulator registers so the readback is in range.
    let nq = (peak as usize).max(2 * n);
    let mut seed = sha3::Shake128::default();
    sha3::digest::Update::update(&mut seed, b"toy-shor-ladder");
    let mut xof = sha3::digest::ExtendableOutput::finalize_xof(seed);
    let mut sim = Simulator::new(nq, nbits as usize, &mut xof);
    sim.clear_for_shot();
    sim.apply_iter(ops.iter());
    (read_reg(&sim, &accx, 0), read_reg(&sim, &accy, 0))
}

#[test]
fn gate_level_ladder_matches_group_law() {
    // Small prime-order curve (order 7); [a]P+[b]Q via chained gate-level point-adds
    // equals the reference group law — the arithmetic oracle for the ADR 0022 toy Shor.
    eprintln!("\n=== capstone (issue #55, ADR 0022): gate-level ladder [a]P+[b]Q ===");
    let curve = ToyCurve::new(7, 0, 5); // y²=x³+5 / F_7, prime order 7
    let (p, ca) = (7u64, 0u64);
    let n = width_for(p);
    let gen = *curve.points.iter().find(|q| !matches!(q, Pt::Inf)).unwrap();
    let secret = 3u64;
    let qbase = ec_mul(secret, gen, ca, p); // Q = [m]P
                                            // pairs chosen to drive ∞ start, doublings, and mid-ladder → ∞.
    let pairs = [
        (0u64, 0u64),
        (1, 0),
        (0, 1),
        (1, 1),
        (2, 3),
        (6, 6),
        (3, 4),
        (5, 2),
        (2, 0),
        (0, 5),
        (6, 1),
        (4, 4),
    ];
    for (a, b) in pairs {
        let got = ladder_eval(a, b, gen, qbase, ca, p, n);
        let want = coords(ec_add(
            ec_mul(a, gen, ca, p),
            ec_mul(b, qbase, ca, p),
            ca,
            p,
        ));
        assert_eq!(
            got, want,
            "gate-level ladder [a]P+[b]Q wrong at a={a}, b={b}"
        );
    }
    eprintln!(
        "  order 7, secret m={secret}: [a]P+[b]Q via chained gate-level point-adds == group law"
    );
    eprintln!(
        "  on {} (a,b) pairs (incl. ∞-start / doubling / →∞). The gate-level arithmetic",
        pairs.len()
    );
    eprintln!("  oracle the ADR 0022 gate-level-QFT toy Shor (toy_shor_qft.py) recovers m from.");
}
