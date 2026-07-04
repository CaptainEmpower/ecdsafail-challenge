//! F2 / hot-path coverage — dump the **actually emitted** `_fast` modular wrapper
//! op-streams (`mod_add_qq_fast`, `mod_sub_qq_fast`, `mod_double_inplace_fast`) so the
//! z3 proof (`analysis/verify/mod_fast_reduction_emitted.py`) can verify the modular
//! arithmetic the **scored hot path** actually runs (58 `_fast` calls, ADR 0027).
//!
//! ADR 0031 bound the *plain* `mod_add_qq` (3 calls) to the emitter. These `_fast`
//! variants fold the same Solinas reduction around the **measurement-based** adder
//! (`cuccaro_add_fast`, HMR + `cz_if`) proved in ADR 0027, so their emitted streams
//! carry HMR/`CZ` ops and free measurement outcomes — the reduction is proved
//! phase-clean *in context*, not just the adder in isolation.
//!
//! Emitted with the **default builder configuration** — the exact configuration
//! `build_circuit` uses, so the proof covers the scored gates. Because the `_fast`
//! wrappers branch on process env vars ([`CONFIG_ENV_VARS`]), the emit is wrapped in a
//! [`DefaultConfigEnv`] guard that clears those vars (and restores them on drop), so an
//! ambient env var can neither make the drift guard fail spuriously nor regenerate a
//! non-scored artifact. Emitting from the real [`B`] builder is the drift guard.
//!
//! `#[cfg(test)]` only; never compiled into the scored circuit (`ops.bin` unchanged).
//! Regenerate:
//! ```text
//! MODFAST_OPS_JSON=analysis/mod_fast_ops.json \
//!   cargo test --release --lib modfast_dump::dump_mod_fast_ops -- --ignored --nocapture
//! ```

use super::{mod_add_qq_fast, mod_double_inplace_fast, mod_sub_qq_fast, B, SECP256K1_P};
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
        other => panic!("modfast_dump: unexpected op kind {other:?}"),
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

const N: usize = 256;

fn ids(v: &[crate::circuit::QubitId]) -> String {
    v.iter()
        .map(|x| x.0.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

/// `mod_add_qq_fast`: `acc := (acc + a) mod p`. Stream object `{op, n, acc, a, ops}`.
fn add_json() -> String {
    let mut b = B::new_for_test();
    let acc = b.alloc_qubits(N);
    let a = b.alloc_qubits(N);
    mod_add_qq_fast(&mut b, &acc, &a, SECP256K1_P);
    let ops = b.take_ops();
    format!(
        "{{\"op\":\"add\",\"n\":{},\"acc\":[{}],\"a\":[{}],\"ops\":{}}}",
        N,
        ids(&acc),
        ids(&a),
        ops_to_json(&ops),
    )
}

/// `mod_sub_qq_fast`: `acc := (acc - a) mod p`. Stream object `{op, n, acc, a, ops}`.
fn sub_json() -> String {
    let mut b = B::new_for_test();
    let acc = b.alloc_qubits(N);
    let a = b.alloc_qubits(N);
    mod_sub_qq_fast(&mut b, &acc, &a, SECP256K1_P);
    let ops = b.take_ops();
    format!(
        "{{\"op\":\"sub\",\"n\":{},\"acc\":[{}],\"a\":[{}],\"ops\":{}}}",
        N,
        ids(&acc),
        ids(&a),
        ops_to_json(&ops),
    )
}

/// `mod_double_inplace_fast`: `v := (2·v) mod p`. Stream object `{op, n, v, ops}`.
fn double_json() -> String {
    let mut b = B::new_for_test();
    let v = b.alloc_qubits(N);
    mod_double_inplace_fast(&mut b, &v, SECP256K1_P);
    let ops = b.take_ops();
    format!(
        "{{\"op\":\"double\",\"n\":{},\"v\":[{}],\"ops\":{}}}",
        N,
        ids(&v),
        ops_to_json(&ops),
    )
}

/// The committed artifact path (repo-root-relative; `cargo test` runs at the
/// workspace root).
const ARTIFACT: &str = "analysis/mod_fast_ops.json";

/// Env vars that toggle emission paths in the `_fast` wrappers / doubling (venting,
/// direct-constant arithmetic, alternative flag-uncompute, carry-truncation). The
/// scored build (`build_circuit`) sets none of these; the dump must emit under that
/// default regardless of the ambient environment, so [`DefaultConfigEnv`] clears them.
const CONFIG_ENV_VARS: &[&str] = &[
    "SECP_DIRECT_CONST_ARITH", // add/sub: direct-constant const-add path
    "KAL_VENT_MODADD",         // add/sub: vented const correction
    "MOD_FAST_FLAG_CONDITIONAL_REPLAY", // add/sub: measurement flag-uncompute variant
    "KAL_DIRECT_CONST_WALKS",  // double: direct_const_walks_enabled()
    "KAL_VENT_DOUBLE",         // double: vented ciadd
    "KAL_DIRECT_CONST_DOUBLE", // double: direct-constant cadd
    "KAL_DOUBLE_CARRY_TRUNC_W", // double: carry-tail-truncated add window
];

/// Serializes env mutation across parallel `cargo test` threads (the default-env
/// toggles are process-global). Poison-tolerant: only mutual exclusion is needed.
static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// RAII: clear every [`CONFIG_ENV_VARS`] entry for its lifetime (saving the prior
/// value) and restore on drop, so the emit runs under the scored/default config no
/// matter what the ambient environment holds. Holds [`ENV_LOCK`] for the duration.
struct DefaultConfigEnv {
    saved: Vec<(&'static str, Option<std::ffi::OsString>)>,
    _lock: std::sync::MutexGuard<'static, ()>,
}

impl DefaultConfigEnv {
    fn enter() -> Self {
        let lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let saved = CONFIG_ENV_VARS
            .iter()
            .map(|&k| {
                let prior = std::env::var_os(k);
                std::env::remove_var(k);
                (k, prior)
            })
            .collect();
        Self { saved, _lock: lock }
    }
}

impl Drop for DefaultConfigEnv {
    fn drop(&mut self) {
        for (k, v) in &self.saved {
            match v {
                Some(val) => std::env::set_var(k, val),
                None => std::env::remove_var(k),
            }
        }
    }
}

/// Build the full JSON string deterministically from the real emitter under the
/// default/scored config. Single source of truth for both the regenerate and the
/// drift-guard tests.
fn build_json() -> String {
    let _env = DefaultConfigEnv::enter();
    format!(
        "{{\"_comment\":\"Emitted _fast modular wrapper op-streams (default config, \
         secp256k1, n=256): mod_add_qq_fast / mod_sub_qq_fast / mod_double_inplace_fast. \
         Regenerate: MODFAST_OPS_JSON=analysis/mod_fast_ops.json cargo test --release \
         --lib modfast_dump::dump_mod_fast_ops -- --ignored\",\"widths\":[{},{},{}]}}\n",
        add_json(),
        sub_json(),
        double_json(),
    )
}

/// Emit the JSON artifact. Ignored by default (writes a file); run with `MODFAST_OPS_JSON`.
#[test]
#[ignore = "regenerates analysis/mod_fast_ops.json; run with MODFAST_OPS_JSON set"]
fn dump_mod_fast_ops() {
    let path =
        std::env::var("MODFAST_OPS_JSON").expect("set MODFAST_OPS_JSON=analysis/mod_fast_ops.json");
    std::fs::write(&path, build_json()).expect("write MODFAST_OPS_JSON");
    eprintln!("modfast_dump: wrote add/sub/double streams to {path}");
}

/// Drift guard (runs in CI): the committed artifact the z3 proof consumes must be
/// byte-identical to a fresh emit. Fails loudly if the emitter changes and the
/// artifact is not regenerated (the model/twin-vs-emitter drift F2 warns about).
#[test]
fn emitted_mod_fast_matches_committed_artifact() {
    let committed = std::fs::read_to_string(ARTIFACT).unwrap_or_else(|e| {
        panic!("cannot read {ARTIFACT} (run from workspace root): {e}");
    });
    assert_eq!(
        committed,
        build_json(),
        "{ARTIFACT} is stale — regenerate with \
         `MODFAST_OPS_JSON={ARTIFACT} cargo test --release --lib \
         modfast_dump::dump_mod_fast_ops -- --ignored`"
    );
}
