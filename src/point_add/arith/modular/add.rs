//! Modular addition over secp256k1 (qq add + vented / fast variants).
use super::*;

/// `acc := (acc + a) mod p`. Both `acc` and `a` are n-bit quantum registers
/// with value in [0, p). Solinas reduction using c = 2^n - p: sum ∈ [0, 2p),
/// then add c, branch on top bit to either clear it (reduction) or undo
/// the add (no reduction). Saves one full (n+1)-wide Cuccaro compared to
/// the sub-p/add-p/csub-p pattern.
pub(crate) fn mod_add_qq(b: &mut B, acc: &[QubitId], a: &[QubitId], p: U256) {
    let n = acc.len();
    assert_eq!(n, a.len());
    debug_assert_eq!(n, 256);

    let (acc_ext, acc_ovf) = ext_reg(b, acc);
    let (a_ext, a_ovf) = ext_reg(b, a);

    // Step 1: (n+1)-bit add. acc_ext ∈ [0, 2p).
    add_nbit_qq(b, &a_ext, &acc_ext);

    // Step 2: add c. If sum was >= p, the top bit of (sum + c) becomes 1.
    let c = U256::MAX.wrapping_sub(p).wrapping_add(U256::from(1));
    add_nbit_const(b, &acc_ext, c);

    // Step 3: flag := acc_ovf (= top bit of sum + c).
    let flag = b.alloc_qubit();
    b.cx(acc_ovf, flag);

    // Step 4: if flag=0 (no reduction needed), undo the add of c.
    b.x(flag);
    csub_nbit_const(b, &acc_ext, c, flag);
    b.x(flag);

    // Step 5: if flag=1, clear the top bit (drops 2^n → yields sum - p).
    b.cx(flag, acc_ovf);

    // Step 6: uncompute flag. Same identity as the old version:
    //   flag == (acc_final < a_orig)
    // because in the flag=1 case acc_final = acc_orig + a - p < a (since acc_orig < p),
    // and in the flag=0 case acc_final = acc_orig + a ≥ a.
    cmp_lt_into(b, &acc_ext[..n], &a_ext[..n], flag);
    b.free(flag);

    unext_reg(b, a_ovf);
    unext_reg(b, acc_ovf);
    let _ = (acc_ext, a_ext);
}

/// Low-peak `acc := (acc + a) mod p`. Identical structure to `mod_add_qq` but
/// the two Solinas-constant corrections (`+c`, conditional `-c`) are vented onto
/// the operand `a_ext` as dirty scratch (2 clean qubits) instead of a fresh
/// n-qubit loaded-constant register. The main add and the flag-uncompute compare
/// stay ancilla-free (Cuccaro / cmp_lt_into), so the only transient is +2 clean.
/// Used inside the round84 Solinas reduction where the materialized `load_const`
/// coexisting with tmp_ext + z1_reg was the peak binder. `c = 2^256 - p` fits in
/// 64 bits, so `c_low` carries the whole constant.
#[allow(dead_code)] // retained reference/alternative impl; not on active build path
pub(crate) fn mod_add_qq_vent(b: &mut B, acc: &[QubitId], a: &[QubitId], p: U256) {
    let n = acc.len();
    assert_eq!(n, a.len());
    debug_assert_eq!(n, 256);

    let (acc_ext, acc_ovf) = ext_reg(b, acc);
    let (a_ext, a_ovf) = ext_reg(b, a);

    add_nbit_qq(b, &a_ext, &acc_ext);

    let c = U256::MAX.wrapping_sub(p).wrapping_add(U256::from(1));
    let c_low = c.as_limbs()[0];
    let n1 = acc_ext.len();
    {
        let q_clean2: [QubitId; 2] = [b.alloc_qubit(), b.alloc_qubit()];
        venting::iadd_dirty_2clean_classical(
            b,
            &acc_ext,
            &a_ext[..n1 - 2],
            &q_clean2,
            c_low,
            false,
        );
        b.free(q_clean2[0]);
        b.free(q_clean2[1]);
    }

    let flag = b.alloc_qubit();
    b.cx(acc_ovf, flag);

    b.x(flag);
    {
        let q_clean2: [QubitId; 2] = [b.alloc_qubit(), b.alloc_qubit()];
        venting::cisub_dirty_2clean_classical(
            b,
            &acc_ext,
            &a_ext[..n1 - 2],
            &q_clean2,
            c_low,
            flag,
        );
        b.free(q_clean2[0]);
        b.free(q_clean2[1]);
    }
    b.x(flag);

    b.cx(flag, acc_ovf);

    cmp_lt_into(b, &acc_ext[..n], &a_ext[..n], flag);
    b.free(flag);

    unext_reg(b, a_ovf);
    unext_reg(b, acc_ovf);
    let _ = (acc_ext, a_ext);
}

// ═══════════════════════════════════════════════════════════════════════════
//  Conditional modular add/sub helpers
// ═══════════════════════════════════════════════════════════════════════════
//
// Used by the multipliers. Each variant loads `(ctrl ? a : 0)` into a
// fresh temporary via CCX or CX_if, runs the unconditional mod_add_qq /
// mod_sub_qq, then unloads.

/// Like `cmp_lt_into` but uses carry-ancilla + measurement-based uncompute
/// for the inv_MAJ sweep. Saves n CCX. NOT emit_inverse-safe.

/// Like `mod_add_qq` but uses `cmp_lt_into_fast` for the flag uncompute.
/// NOT safe inside emit_inverse blocks.
pub(crate) fn mod_add_qq_fast(b: &mut B, acc: &[QubitId], a: &[QubitId], p: U256) {
    let n = acc.len();
    assert_eq!(n, a.len());
    debug_assert_eq!(n, 256);

    let (acc_ext, acc_ovf) = ext_reg(b, acc);
    let (a_ext, a_ovf) = ext_reg(b, a);

    // Use fast (measurement-based) Cuccaro everywhere.
    add_nbit_qq_fast(b, &a_ext, &acc_ext);
    let c = U256::MAX.wrapping_sub(p).wrapping_add(U256::from(1));
    // add_nbit_const with fast Cuccaro OR venting (using `a` as dirty).
    let use_vent = kal_vent_modadd_enabled();
    if use_vent {
        let n1 = acc_ext.len();
        // Use `a_ext` as dirty qubits (it was just used as add operand,
        // its value is preserved through the venting sub-protocol).
        let c_low = c.as_limbs()[0];
        let q_clean2: [QubitId; 2] = [b.alloc_qubit(), b.alloc_qubit()];
        venting::iadd_dirty_2clean_classical(
            b,
            &acc_ext,
            &a_ext[..n1 - 2],
            &q_clean2,
            c_low,
            false,
        );
        b.free(q_clean2[0]);
        b.free(q_clean2[1]);
    } else if secp_direct_const_arith_enabled() {
        add_nbit_const_direct_uncontrolled_fast(b, &acc_ext, c);
    } else {
        let n1 = acc_ext.len();
        let ca = load_const(b, n1, c);
        add_nbit_qq_fast(b, &ca, &acc_ext);
        unload_const(b, &ca, c);
    }
    let flag = b.alloc_qubit();
    b.cx(acc_ovf, flag);
    b.x(flag);
    // csub_nbit_const with fast Cuccaro OR venting.
    if use_vent {
        let c_low = c.as_limbs()[0];
        let n1 = acc_ext.len();
        let q_clean2: [QubitId; 2] = [b.alloc_qubit(), b.alloc_qubit()];
        venting::cisub_dirty_2clean_classical(
            b,
            &acc_ext,
            &a_ext[..n1 - 2],
            &q_clean2,
            c_low,
            flag,
        );
        b.free(q_clean2[0]);
        b.free(q_clean2[1]);
    } else if secp_direct_const_arith_enabled() {
        csub_nbit_const_direct_fast(b, &acc_ext, c, flag);
    } else {
        let n1 = acc_ext.len();
        let ca = b.alloc_qubits(n1);
        for i in 0..n1 {
            if bit(c, i) {
                b.cx(flag, ca[i]);
            }
        }
        sub_nbit_qq_fast(b, &ca, &acc_ext);
        for i in 0..n1 {
            if bit(c, i) {
                b.cx(flag, ca[i]);
            }
        }
        b.free_vec(&ca);
    }
    b.x(flag);
    b.cx(flag, acc_ovf);
    if std::env::var("MOD_FAST_FLAG_CONDITIONAL_REPLAY")
        .ok()
        .as_deref()
        == Some("1")
    {
        let phase = b.alloc_bit();
        b.hmr(flag, phase);
        cmp_lt_phase_conditioned(b, &acc_ext[..n], &a_ext[..n], phase);
    } else {
        cmp_lt_into_fast(b, &acc_ext[..n], &a_ext[..n], flag);
    }
    b.free(flag);

    unext_reg(b, a_ovf);
    unext_reg(b, acc_ovf);
    let _ = (acc_ext, a_ext);
}

/// Specialization of mod_add_qq_fast when acc = 0 on entry. Replaces the
/// initial Cuccaro add with CX-copy (0 CCX instead of n-1 CCX).
/// Saves 255 CCX per call.
pub(crate) fn mod_add_qq_fast_from_zero(b: &mut B, acc: &[QubitId], a: &[QubitId], p: U256) {
    let n = acc.len();
    assert_eq!(n, a.len());
    debug_assert_eq!(n, 256);

    let (acc_ext, acc_ovf) = ext_reg(b, acc);
    let (a_ext, a_ovf) = ext_reg(b, a);

    // acc is 0 on entry. CX-copy a into acc (0 CCX). Top bits both 0.
    for i in 0..n {
        b.cx(a[i], acc[i]);
    }
    // acc_ovf and a_ovf are both 0 (both freshly allocated as 0 by ext_reg).

    let c = U256::MAX.wrapping_sub(p).wrapping_add(U256::from(1));
    let use_vent = kal_vent_modadd_enabled();
    if use_vent {
        let n1 = acc_ext.len();
        let c_low = c.as_limbs()[0];
        let q_clean2: [QubitId; 2] = [b.alloc_qubit(), b.alloc_qubit()];
        venting::iadd_dirty_2clean_classical(
            b,
            &acc_ext,
            &a_ext[..n1 - 2],
            &q_clean2,
            c_low,
            false,
        );
        b.free(q_clean2[0]);
        b.free(q_clean2[1]);
    } else {
        let n1 = acc_ext.len();
        let ca = load_const(b, n1, c);
        add_nbit_qq_fast(b, &ca, &acc_ext);
        unload_const(b, &ca, c);
    }
    let flag = b.alloc_qubit();
    b.cx(acc_ovf, flag);
    b.x(flag);
    if use_vent {
        let c_low = c.as_limbs()[0];
        let n1 = acc_ext.len();
        let q_clean2: [QubitId; 2] = [b.alloc_qubit(), b.alloc_qubit()];
        venting::cisub_dirty_2clean_classical(
            b,
            &acc_ext,
            &a_ext[..n1 - 2],
            &q_clean2,
            c_low,
            flag,
        );
        b.free(q_clean2[0]);
        b.free(q_clean2[1]);
    } else {
        let n1 = acc_ext.len();
        let ca = b.alloc_qubits(n1);
        for i in 0..n1 {
            if bit(c, i) {
                b.cx(flag, ca[i]);
            }
        }
        sub_nbit_qq_fast(b, &ca, &acc_ext);
        for i in 0..n1 {
            if bit(c, i) {
                b.cx(flag, ca[i]);
            }
        }
        b.free_vec(&ca);
    }
    b.x(flag);
    b.cx(flag, acc_ovf);
    if std::env::var("MOD_FAST_FLAG_CONDITIONAL_REPLAY")
        .ok()
        .as_deref()
        == Some("1")
    {
        let phase = b.alloc_bit();
        b.hmr(flag, phase);
        cmp_lt_phase_conditioned(b, &acc_ext[..n], &a_ext[..n], phase);
    } else {
        cmp_lt_into_fast(b, &acc_ext[..n], &a_ext[..n], flag);
    }
    b.free(flag);

    unext_reg(b, a_ovf);
    unext_reg(b, acc_ovf);
    let _ = (acc_ext, a_ext);
}

#[cfg(test)]
mod tests {
    use super::super::test_util::*;
    use super::*;
    use crate::point_add::SECP256K1_P;
    use crate::sim::Simulator;
    use sha3::{
        digest::{ExtendableOutput, Update},
        Shake128,
    };

    /// `mod_add_qq`: `acc := (acc + a) mod p`, `a` preserved, every ancilla back to
    /// |0>. 64 random `(x, y) < p` across masked shots.
    #[test]
    fn mod_add_qq_is_x_plus_y_mod_p() {
        let p = SECP256K1_P;
        let mut b = B::new();
        let acc = b.alloc_qubits(256);
        let a = b.alloc_qubits(256);
        mod_add_qq(&mut b, &acc, &a, p);
        let nq = b.next_qubit as usize;
        let nb = b.next_bit as usize;
        let inputs: std::collections::HashSet<u64> =
            acc.iter().chain(a.iter()).map(|q| q.0).collect();
        let mut seed = Shake128::default();
        seed.update(b"mod_add_qq");
        let mut xof = seed.finalize_xof();
        let mut sim = Simulator::new(nq, nb, &mut xof);
        let xs: Vec<U256> = (0..64).map(|s| rand_lt_p(s as u64 * 2)).collect();
        let ys: Vec<U256> = (0..64).map(|s| rand_lt_p(s as u64 * 2 + 1)).collect();
        for s in 0..64 {
            set256(&mut sim, &acc, xs[s], s);
            set256(&mut sim, &a, ys[s], s);
        }
        sim.apply_iter(b.ops.iter());
        assert_eq!(sim.phase, 0, "phase garbage");
        for s in 0..64 {
            let (x, y) = (xs[s], ys[s]);
            let exp = if x >= p - y { x - (p - y) } else { x + y };
            assert_eq!(get256(&sim, &acc, s), exp, "acc != (x+y) mod p, shot {s}");
            assert_eq!(get256(&sim, &a, s), y, "a changed, shot {s}");
        }
        for q in 0..nq as u64 {
            if !inputs.contains(&q) {
                assert_eq!(sim.qubit(QubitId(q)), 0, "ancilla q{q} not clean");
            }
        }
    }
}
