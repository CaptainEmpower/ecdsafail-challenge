//! F1/F2 (issue referee-review, ADR 0027) — dump the **actually emitted**
//! `cuccaro_add_fast` op-stream to JSON so the z3 proof
//! (`analysis/verify/mbuc_phase_correction.py`) can symbolically verify the
//! measurement-based uncompute (HMR + `cz_if` phase correction) the scored circuit
//! runs — not a re-implementation.
//!
//! The scored hot path emits `mod_add_qq_fast` / `mod_sub_qq_fast` /
//! `mod_double_inplace_fast`, all of which route their addition through
//! [`cuccaro_add_fast`] (`arith/adder.rs`) and its measurement-based UMA sweep
//! (`hmr` + `cz_if`). The z3 (`solinas_reduction.py`) and Kani proofs model only the
//! *plain* `mod_add_qq` and treat adders as exact integer `+`; the HMR/CZ
//! phase-kickback logic has **zero symbolic coverage** and is guarded only by the
//! 9024-shot sample (referee findings F1/F2, `paper/REVIEW.md`).
//!
//! This harness emits the real op-stream of `cuccaro_add_fast` at several widths
//! (including the production 256) to a JSON artifact that the Python z3 proof
//! consumes. Emitting from the real [`B`] builder is the drift guard: the proof
//! verifies the emitter's output, so a divergence between "the copy" and the emitted
//! gates cannot pass silently (the exact gap F2 names).
//!
//! `#[cfg(test)]` only; never compiled into the scored circuit (`ops.bin` unchanged).
//! Regenerate the artifact with:
//! ```text
//! MBUC_OPS_JSON=analysis/mbuc_fast_adder_ops.json \
//!   cargo test --release --lib mbuc_dump::dump_fast_adder_ops -- --ignored --nocapture
//! ```

use super::{cuccaro_add_fast, B};
use crate::circuit::{Op, OperationType, NO_BIT, NO_QUBIT};

/// Widths dumped: small ones for cheap exhaustive-adjacent proofs, plus the
/// production 256-bit coordinate register width.
const WIDTHS: &[usize] = &[2, 3, 4, 8, 16, 64, 256];

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
        other => panic!("mbuc_dump: unexpected op kind {other:?} in fast adder"),
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

fn one_width_json(n: usize) -> String {
    let mut b = B::new_for_test();
    let a = b.alloc_qubits(n);
    let acc = b.alloc_qubits(n);
    let c_in = b.alloc_qubit();
    cuccaro_add_fast(&mut b, &a, &acc, c_in);
    let ops = b.take_ops();
    let ids = |v: &[crate::circuit::QubitId]| {
        v.iter()
            .map(|x| x.0.to_string())
            .collect::<Vec<_>>()
            .join(",")
    };
    format!(
        "{{\"n\":{},\"a\":[{}],\"acc\":[{}],\"c_in\":{},\"ops\":{}}}",
        n,
        ids(&a),
        ids(&acc),
        c_in.0,
        ops_to_json(&ops),
    )
}

/// The committed artifact path (repo-root-relative; `cargo test` runs at the
/// workspace root).
const ARTIFACT: &str = "analysis/mbuc_fast_adder_ops.json";

/// Build the full JSON string deterministically from the real emitter. Single
/// source of truth for both the regenerate and the drift-guard tests.
fn build_json() -> String {
    let bodies: Vec<String> = WIDTHS.iter().map(|&n| one_width_json(n)).collect();
    format!(
        "{{\"_comment\":\"Emitted cuccaro_add_fast op-streams (ADR 0027, F1/F2). \
         Regenerate: MBUC_OPS_JSON=analysis/mbuc_fast_adder_ops.json cargo test --release \
         --lib mbuc_dump::dump_fast_adder_ops -- --ignored\",\"widths\":[{}]}}\n",
        bodies.join(",")
    )
}

/// Emit the JSON artifact consumed by `mbuc_phase_correction.py`. Ignored by
/// default (writes a file); run explicitly with `MBUC_OPS_JSON` set.
#[test]
#[ignore = "regenerates analysis/mbuc_fast_adder_ops.json; run with MBUC_OPS_JSON set"]
fn dump_fast_adder_ops() {
    let path = std::env::var("MBUC_OPS_JSON")
        .expect("set MBUC_OPS_JSON=analysis/mbuc_fast_adder_ops.json");
    std::fs::write(&path, build_json()).expect("write MBUC_OPS_JSON");
    eprintln!("mbuc_dump: wrote {} widths to {path}", WIDTHS.len());
}

/// Drift guard (runs in CI): the committed artifact the z3 proof consumes must be
/// byte-identical to a fresh emit of `cuccaro_add_fast`. If the emitter changes and
/// the artifact is not regenerated, the proof would silently verify stale gates — the
/// exact "copy vs emitter drift" failure F2 warns about. This fails loudly instead.
#[test]
fn emitted_ops_match_committed_artifact() {
    let committed = std::fs::read_to_string(ARTIFACT).unwrap_or_else(|e| {
        panic!("cannot read {ARTIFACT} (run from workspace root): {e}");
    });
    assert_eq!(
        committed,
        build_json(),
        "{ARTIFACT} is stale — regenerate with \
         `MBUC_OPS_JSON={ARTIFACT} cargo test --release --lib \
         mbuc_dump::dump_fast_adder_ops -- --ignored`"
    );
}
