//! Modular subtraction over secp256k1 (qq sub + fast / vented variants).
use super::*;

#[allow(dead_code)] // retained reference/alternative impl; not on active build path
pub(crate) fn mod_sub_qq(b: &mut B, acc: &[QubitId], a: &[QubitId], p: U256) {
    // mod_add_qq is a bijection on (acc, a): (acc, a) ↦ (acc + a mod p, a).
    // Its gate-level inverse therefore acts as (acc, a) ↦ (acc - a mod p, a),
    // which is exactly what we want. emit_inverse replays the forward's gates
    // reversed, skipping R markers — valid because mod_add_qq is clean
    // (every ancilla is driven to |0⟩ before its R).
    let a_copy: Vec<QubitId> = a.to_vec();
    emit_inverse(b, move |b| mod_add_qq(b, acc, &a_copy, p));
}

/// Fast `acc := (acc - a) mod p`. Direct sub + conditional add-p + flag
/// uncompute via neg+cmp_lt+neg. All ops use measurement-based Cuccaro.
pub(crate) fn mod_sub_qq_fast(b: &mut B, acc: &[QubitId], a: &[QubitId], p: U256) {
    let n = acc.len();
    assert_eq!(n, a.len());
    debug_assert_eq!(n, 256);

    let (acc_ext, acc_ovf) = ext_reg(b, acc);
    let (a_ext, a_ovf) = ext_reg(b, a);

    // Step 1: (n+1)-bit sub.
    sub_nbit_qq_fast(b, &a_ext, &acc_ext);

    // Step 2: flag = acc_ovf (=1 iff underflow, i.e. acc < a).
    let flag = b.alloc_qubit();
    b.cx(acc_ovf, flag);
    // We only need the borrow as a separate flag; the low register is
    // corrected modulo 2^n, so clear the extension bit immediately.
    b.cx(flag, acc_ovf);

    // Step 3: underflow correction. With p = 2^n - c, the wrapped 256-bit
    // subtraction needs only a conditional subtract of c on the low register.
    let c = U256::MAX.wrapping_sub(p).wrapping_add(U256::from(1));
    if kal_vent_modadd_enabled() {
        // Use venting cisub with a_ext as dirty qubits.
        let c_low = c.as_limbs()[0];
        let q_clean2: [QubitId; 2] = [b.alloc_qubit(), b.alloc_qubit()];
        venting::cisub_dirty_2clean_classical(
            b,
            &acc_ext[..n],
            &a_ext[..n - 2],
            &q_clean2,
            c_low,
            flag,
        );
        b.free(q_clean2[0]);
        b.free(q_clean2[1]);
    } else if secp_direct_const_arith_enabled() {
        csub_nbit_const_direct_fast(b, &acc_ext[..n], c, flag);
    } else {
        csub_nbit_const_fast(b, &acc_ext[..n], c, flag);
    }

    // Step 4: uncompute flag. Identity: flag = NOT(acc_final < (p - a)).
    // Negate a in place, compare, un-negate.
    b.x(flag);
    mod_neg_inplace_fast(b, &a_ext[..n], p);
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
    mod_neg_inplace_fast(b, &a_ext[..n], p);
    b.free(flag);

    unext_reg(b, a_ovf);
    unext_reg(b, acc_ovf);
    let _ = (acc_ext, a_ext);
}

/// `acc := (acc - a) mod p`, low-peak. Explicit gate-reverse of
/// `mod_add_qq_vent` (the venting protocols use measurement, so `emit_inverse`
/// cannot reverse them; each venting step is undone by its matched dual:
/// iadd↔isub, cisub↔ciadd). The flag-uncompute is `cmp_lt_into` (self-inverse,
/// no materialized neg), so no n-wide const register is ever live.
#[allow(dead_code)] // retained reference/alternative impl; not on active build path
pub(crate) fn mod_sub_qq_vent(b: &mut B, acc: &[QubitId], a: &[QubitId], p: U256) {
    let n = acc.len();
    assert_eq!(n, a.len());
    debug_assert_eq!(n, 256);

    let (acc_ext, acc_ovf) = ext_reg(b, acc);
    let (a_ext, a_ovf) = ext_reg(b, a);

    let c = U256::MAX.wrapping_sub(p).wrapping_add(U256::from(1));
    let c_low = c.as_limbs()[0];
    let n1 = acc_ext.len();

    // Reverse of forward step 6: cmp_lt_into is its own inverse (XOR into flag).
    let flag = b.alloc_qubit();
    cmp_lt_into(b, &acc_ext[..n], &a_ext[..n], flag);

    // Reverse of step 5.
    b.cx(flag, acc_ovf);

    // Reverse of step 4: forward applied (cisub c) under !flag; undo with ciadd.
    b.x(flag);
    {
        let q_clean2: [QubitId; 2] = [b.alloc_qubit(), b.alloc_qubit()];
        venting::ciadd_dirty_2clean_classical(
            b,
            &acc_ext,
            &a_ext[..n1 - 2],
            &q_clean2,
            c_low,
            flag,
            false,
        );
        b.free(q_clean2[0]);
        b.free(q_clean2[1]);
    }
    b.x(flag);

    // Reverse of step 3.
    b.cx(acc_ovf, flag);
    b.free(flag);

    // Reverse of step 2: undo the unconditional (iadd c) with a cisub under an
    // always-on control.
    {
        let one = b.alloc_qubit();
        b.x(one);
        let q_clean2: [QubitId; 2] = [b.alloc_qubit(), b.alloc_qubit()];
        venting::cisub_dirty_2clean_classical(b, &acc_ext, &a_ext[..n1 - 2], &q_clean2, c_low, one);
        b.free(q_clean2[0]);
        b.free(q_clean2[1]);
        b.x(one);
        b.free(one);
    }

    // Reverse of step 1.
    sub_nbit_qq(b, &a_ext, &acc_ext);

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

    /// `mod_sub_qq`: `acc := (acc − a) mod p`, `a` preserved, ancilla clean.
    #[test]
    fn mod_sub_qq_is_x_minus_y_mod_p() {
        let p = SECP256K1_P;
        let mut b = B::new();
        let acc = b.alloc_qubits(256);
        let a = b.alloc_qubits(256);
        mod_sub_qq(&mut b, &acc, &a, p);
        let nq = b.next_qubit as usize;
        let nb = b.next_bit as usize;
        let inputs: std::collections::HashSet<u64> =
            acc.iter().chain(a.iter()).map(|q| q.0).collect();
        let mut seed = Shake128::default();
        seed.update(b"mod_sub_qq");
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
            let exp = if x >= y { x - y } else { p - (y - x) };
            assert_eq!(get256(&sim, &acc, s), exp, "acc != (x-y) mod p, shot {s}");
            assert_eq!(get256(&sim, &a, s), y, "a changed, shot {s}");
        }
        for q in 0..nq as u64 {
            if !inputs.contains(&q) {
                assert_eq!(sim.qubit(QubitId(q)), 0, "ancilla q{q} not clean");
            }
        }
    }
}
