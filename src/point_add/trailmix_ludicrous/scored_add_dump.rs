//! Dump the **scored** adder's emitted op-stream so the z3 proof
//! (`analysis/verify/scored_add_emitted.py`) can verify it over the actual gates
//! `ops.bin` runs — closing the gap ADR 0035 named (the emitter-bound arithmetic
//! proofs bind *reference* primitives; the scored circuit is `trailmix_ludicrous`).
//!
//! `arith::hybrid_add_adaptive(circ, a, b, k)` is the adder the scored square's
//! `add_into` drives (`square.rs`): `a := (a + b) mod 2^n`, `b` preserved. It is
//! deterministic in `(n, k)` (the layout comes from `adaptive_layout(n,k)` /
//! `hybrid_add_plain`; no `active_qubits` read), so a fresh-builder emit reproduces the
//! scored gates for that `(n, k)`. Widths/`k` here span **both** dispatch branches — the
//! plain Gidney add (`k + 2√n ≥ n`) and the sqrt(n)-chunked add — at widths up to the
//! production 256/258, so the proof covers the regimes the square schedule drives.
//!
//! `#[cfg(test)]` only; never compiled into `build_circuit` (`ops.bin` unchanged).
//! Regenerate:
//! ```text
//! SCORED_ADD_OPS_JSON=analysis/scored_add_ops.json \
//!   cargo test --release --lib trailmix_ludicrous::scored_add_dump::dump_scored_add_ops -- --ignored
//! ```

use super::arith::hybrid_add_adaptive;
use super::B;
use crate::circuit::{Op, OperationType, NO_BIT, NO_QUBIT};

fn op_name(k: OperationType) -> &'static str {
    match k {
        OperationType::Neg => "NEG",
        OperationType::X => "X",
        OperationType::Z => "Z",
        OperationType::CX => "CX",
        OperationType::CZ => "CZ",
        OperationType::Swap => "SWAP",
        OperationType::R => "R",
        OperationType::Hmr => "HMR",
        OperationType::CCX => "CCX",
        OperationType::CCZ => "CCZ",
        OperationType::BitInvert => "BIT_INVERT",
        OperationType::BitStore0 => "BIT_STORE0",
        OperationType::BitStore1 => "BIT_STORE1",
        OperationType::PushCondition => "PUSH_CONDITION",
        OperationType::PopCondition => "POP_CONDITION",
        other => panic!("scored_add_dump: unexpected op kind {other:?}"),
    }
}

fn q(id: crate::circuit::QubitId) -> i64 {
    if id == NO_QUBIT {
        -1
    } else {
        id.0 as i64
    }
}

fn c(id: crate::circuit::BitId) -> i64 {
    if id == NO_BIT {
        -1
    } else {
        id.0 as i64
    }
}

fn ops_to_json(ops: &[Op]) -> String {
    let mut s = String::from("[");
    for (i, op) in ops.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        // [kind, q_control2, q_control1, q_target, c_target, c_condition]
        s.push_str(&format!(
            "[\"{}\",{},{},{},{},{}]",
            op_name(op.kind),
            q(op.q_control2),
            q(op.q_control1),
            q(op.q_target),
            c(op.c_target),
            c(op.c_condition),
        ));
    }
    s.push(']');
    s
}

/// `(n, k)` configs spanning both dispatch branches. Plain (`k ≥ n` ⇒ `k+2√n ≥ n`) and
/// chunked (`k` chosen `≥ ⌈n/c⌉+c+ADAPTIVE_RES` so the tight/unreachable sub-branch never
/// fires, and `k+2c < n`). Small widths are cheap thorough checks; 256 is production.
const CONFIGS: &[(usize, usize)] = &[
    // plain branch
    (4, 10),
    (8, 12),
    (16, 24),
    (64, 80),
    (128, 160),
    (256, 300),
    // chunked branch
    (64, 32),
    (128, 40),
    (256, 64),
];

fn ids(v: &[crate::circuit::QubitId]) -> String {
    v.iter()
        .map(|x| x.0.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

/// Emit `a := (a + b) mod 2^n` on two fresh n-bit registers via the scored adder.
fn one_config_json(n: usize, k: usize) -> String {
    let mut b = B::new_for_test();
    let a = b.alloc_qubits(n);
    let addend = b.alloc_qubits(n);
    hybrid_add_adaptive(&mut b, &a, &addend, k);
    let ops = b.take_ops();
    format!(
        "{{\"n\":{},\"k\":{},\"a\":[{}],\"b\":[{}],\"ops\":{}}}",
        n,
        k,
        ids(&a),
        ids(&addend),
        ops_to_json(&ops),
    )
}

const ARTIFACT: &str = "analysis/scored_add_ops.json";

fn build_json() -> String {
    let bodies: Vec<String> = CONFIGS
        .iter()
        .map(|&(n, k)| one_config_json(n, k))
        .collect();
    format!(
        "{{\"_comment\":\"Emitted trailmix_ludicrous scored adder (hybrid_add_adaptive) \
         op-streams (ADR 0036). a:=(a+b) mod 2^n. Regenerate: \
         SCORED_ADD_OPS_JSON=analysis/scored_add_ops.json cargo test --release --lib \
         trailmix_ludicrous::scored_add_dump::dump_scored_add_ops -- --ignored\",\"widths\":[{}]}}\n",
        bodies.join(",")
    )
}

#[test]
#[ignore = "regenerates analysis/scored_add_ops.json; run with SCORED_ADD_OPS_JSON set"]
fn dump_scored_add_ops() {
    let path = std::env::var("SCORED_ADD_OPS_JSON")
        .expect("set SCORED_ADD_OPS_JSON=analysis/scored_add_ops.json");
    std::fs::write(&path, build_json()).expect("write SCORED_ADD_OPS_JSON");
    eprintln!("scored_add_dump: wrote {} configs to {path}", CONFIGS.len());
}

/// Drift guard (runs in CI): the committed artifact the z3 proof consumes must be
/// byte-identical to a fresh emit of `hybrid_add_adaptive`.
#[test]
fn emitted_scored_add_matches_committed_artifact() {
    let committed = std::fs::read_to_string(ARTIFACT).unwrap_or_else(|e| {
        panic!("cannot read {ARTIFACT} (run from workspace root): {e}");
    });
    assert_eq!(
        committed,
        build_json(),
        "{ARTIFACT} is stale — regenerate with \
         `SCORED_ADD_OPS_JSON={ARTIFACT} cargo test --release --lib \
         trailmix_ludicrous::scored_add_dump::dump_scored_add_ops -- --ignored`"
    );
}
