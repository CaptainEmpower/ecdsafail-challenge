//! F2 / Solinas-reduction coverage — dump the **actually emitted** `mod_add_qq`
//! op-stream to JSON so the z3 proof (`analysis/verify/solinas_reduction_emitted.py`)
//! can verify the modular reduction over the *emitted gates*, not a model.
//!
//! The existing Solinas-reduction proofs both re-implement the algorithm:
//! `analysis/verify/solinas_reduction.py` mirrors it "step-for-step" in z3 BitVec,
//! and `src/kani_proofs.rs::solinas_add` is a hand-written integer twin (the exact
//! copy referee finding F2 warns about). ADR 0027/0030 bound the underlying
//! `cuccaro_add_fast` *adder* to the emitter (z3 + Kani), but the modular-reduction
//! *wrapper* — add `c = 2^256 − p`, branch on the overflow, undo or clear, uncompute
//! the flag — was still covered only by re-implementations.
//!
//! This harness emits the real `mod_add_qq` op-stream (`arith/modular/add.rs`) at the
//! production 256-bit secp256k1 width and serializes it to a JSON artifact the z3
//! proof consumes. Emitting from the real [`B`] builder is the drift guard: the proof
//! verifies the emitter's output, so a divergence between "the model/twin" and the
//! emitted gates cannot pass silently.
//!
//! `#[cfg(test)]` only; never compiled into the scored circuit (`ops.bin` unchanged).
//! Regenerate the artifact with:
//! ```text
//! MODADD_OPS_JSON=analysis/mod_add_qq_ops.json \
//!   cargo test --release --lib modadd_dump::dump_mod_add_qq_ops -- --ignored --nocapture
//! ```
//!
//! NOTE: `mod_add_qq` hard-codes `c = 2^256 − p` (`U256::MAX − p + 1`), so it is only
//! correct at `n = 256` (its own `debug_assert`); this is why the artifact is a single
//! 256-bit width, unlike the multi-width `mbuc_dump`.

use super::{mod_add_qq, B, SECP256K1_P};
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
        other => panic!("modadd_dump: unexpected op kind {other:?} in mod_add_qq"),
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

/// Production width; `mod_add_qq`'s Solinas constant is baked for `n = 256`.
const N: usize = 256;

fn ids(v: &[crate::circuit::QubitId]) -> String {
    v.iter()
        .map(|x| x.0.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

/// Emit `acc := (acc + a) mod p` on two fresh 256-bit registers and return the
/// JSON stream object `{n, acc, a, ops}` (the `proof_toolkit.load_streams` format;
/// `acc`/`a` are the input-register qubit ids).
fn one_width_json() -> String {
    let mut b = B::new_for_test();
    let acc = b.alloc_qubits(N);
    let a = b.alloc_qubits(N);
    mod_add_qq(&mut b, &acc, &a, SECP256K1_P);
    let ops = b.take_ops();
    format!(
        "{{\"n\":{},\"acc\":[{}],\"a\":[{}],\"ops\":{}}}",
        N,
        ids(&acc),
        ids(&a),
        ops_to_json(&ops),
    )
}

/// The committed artifact path (repo-root-relative; `cargo test` runs at the
/// workspace root).
const ARTIFACT: &str = "analysis/mod_add_qq_ops.json";

/// Build the full JSON string deterministically from the real emitter. Single
/// source of truth for both the regenerate and the drift-guard tests.
fn build_json() -> String {
    format!(
        "{{\"_comment\":\"Emitted mod_add_qq op-stream (Solinas reduction, secp256k1, \
         n=256). Regenerate: MODADD_OPS_JSON=analysis/mod_add_qq_ops.json cargo test \
         --release --lib modadd_dump::dump_mod_add_qq_ops -- --ignored\",\"widths\":[{}]}}\n",
        one_width_json()
    )
}

/// Emit the JSON artifact consumed by `solinas_reduction_emitted.py`. Ignored by
/// default (writes a file); run explicitly with `MODADD_OPS_JSON` set.
#[test]
#[ignore = "regenerates analysis/mod_add_qq_ops.json; run with MODADD_OPS_JSON set"]
fn dump_mod_add_qq_ops() {
    let path =
        std::env::var("MODADD_OPS_JSON").expect("set MODADD_OPS_JSON=analysis/mod_add_qq_ops.json");
    std::fs::write(&path, build_json()).expect("write MODADD_OPS_JSON");
    eprintln!("modadd_dump: wrote n={N} mod_add_qq stream to {path}");
}

/// Drift guard (runs in CI): the committed artifact the z3 proof consumes must be
/// byte-identical to a fresh emit of `mod_add_qq`. If the emitter changes and the
/// artifact is not regenerated, the proof would silently verify stale gates — the
/// exact "model/twin vs emitter drift" failure F2 warns about. This fails loudly.
#[test]
fn emitted_mod_add_qq_matches_committed_artifact() {
    let committed = std::fs::read_to_string(ARTIFACT).unwrap_or_else(|e| {
        panic!("cannot read {ARTIFACT} (run from workspace root): {e}");
    });
    assert_eq!(
        committed,
        build_json(),
        "{ARTIFACT} is stale — regenerate with \
         `MODADD_OPS_JSON={ARTIFACT} cargo test --release --lib \
         modadd_dump::dump_mod_add_qq_ops -- --ignored`"
    );
}
