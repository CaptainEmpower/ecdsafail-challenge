//! Modular scaling: negate, double, and halve (in-place, fast variants).
use super::*;

/// Fast mod_neg using measurement-based Cuccaro for the addition.
pub(crate) fn mod_neg_inplace_fast(b: &mut B, v: &[QubitId], p: U256) {
    for &q in v {
        b.x(q);
    }
    let n = v.len();
    let ca = load_const(b, n, p.wrapping_add(U256::from(1)));
    add_nbit_qq_fast(b, &ca, v);
    unload_const(b, &ca, p.wrapping_add(U256::from(1)));
}

/// `tx := (Qx - tx) mod p`, `Qx` the classical bit register `bits`, `tx` in [0,p).
/// FUSE_X_RESTORE primitive: fuses the x-restore chain `[neg, +Qx]` into one
/// "constant-minus-register" modular op, folding the negation's reduction into the
/// subtract's own underflow fold (one reduction instead of two). Mirrors the existing
/// vented controlled-subtract pattern in this file (see mod_sub_qq_vent).
pub(crate) fn mod_const_minus_reg_qb(b: &mut B, tx: &[QubitId], bits: &[BitId], p: U256) {
    let n = tx.len();
    assert_eq!(n, bits.len());
    let a = load_bits(b, bits); // Qx (preserved as uncompute operand)
    let (a_ext, a_ovf) = ext_reg(b, &a);
    let (tx_ext, tx_ovf) = ext_reg(b, tx);
    for i in 0..n {
        b.x(tx_ext[i]); // ~tx = 2^n-1-tx
    }
    let cin = b.alloc_qubit();
    b.x(cin); // +1 carry-in
    cuccaro_add_low_to_ext_clean(b, &a, &tx_ext, cin); // tx_ext = 2^n + (Qx - tx)
    b.x(cin);
    b.free(cin);
    let flag = b.alloc_qubit(); // flag = carry = (Qx >= tx)
    b.cx(tx_ovf, flag);
    b.cx(flag, tx_ovf); // capture + clear the 2^n bit
    let c = U256::MAX.wrapping_sub(p).wrapping_add(U256::from(1));
    let c_low = c.as_limbs()[0];
    let n1 = tx_ext.len();
    b.x(flag); // if underflow (Qx<tx): subtract c (= +p fold)
    {
        let q2: [QubitId; 2] = [b.alloc_qubit(), b.alloc_qubit()];
        venting::cisub_dirty_2clean_classical(b, &tx_ext, &a_ext[..n1 - 2], &q2, c_low, flag);
        b.free(q2[0]);
        b.free(q2[1]);
    }
    b.x(flag);
    b.x(flag); // uncompute flag via clean cmp_lt (Qx still live)
    cmp_lt_into(b, &a, &tx_ext[..n], flag);
    b.free(flag);
    unext_reg(b, tx_ovf);
    unext_reg(b, a_ovf);
    let _ = (tx_ext, a_ext);
    unload_bits(b, &a, bits);
}

// ═══════════════════════════════════════════════════════════════════════════
//  Non-modular n-bit primitives
// ═══════════════════════════════════════════════════════════════════════════

/// Fast Cuccaro sub: `acc -= a mod 2^n` with measurement UMA (0 Toffoli
/// for UMA sweep). Exact gate-level inverse of `cuccaro_add_fast`.
/// Fast `acc += a mod 2^n` using measurement-based Cuccaro.

pub(crate) fn mod_double_inplace_fast(b: &mut B, v: &[QubitId], p: U256) {
    mod_double_inplace_fast_with_dirty(b, v, p, None)
}

pub(crate) fn mod_double_inplace_fast_with_dirty(
    b: &mut B,
    v: &[QubitId],
    p: U256,
    dirty_src: Option<&[QubitId]>,
) {
    let n = v.len();
    let ovf = b.alloc_qubit();
    b.swap(v[n - 1], ovf);
    for i in (0..n - 1).rev() {
        b.swap(v[i], v[i + 1]);
    }
    debug_assert_eq!(n, 256);
    // For secp256k1, p = 2^n - c. After the shift, the old top bit is in
    // `ovf` and the low register holds T mod 2^n for T = 2*v. If ovf=1 then
    // T = 2^n + low and T mod p = low + c; otherwise T mod p = low.
    let c = U256::MAX.wrapping_sub(p).wrapping_add(U256::from(1));
    let use_venting = std::env::var("KAL_VENT_DOUBLE").ok().as_deref() == Some("1")
        && dirty_src.map_or(false, |d| d.len() >= n - 2);
    if let Some(w) = double_carry_trunc_window() {
        // Carry-tail-truncated sparse-constant add (default OFF).
        cadd_nbit_const_direct_trunc_fast(b, v, c, ovf, w);
    } else if use_venting {
        let dirty = dirty_src.unwrap();
        let q_clean2: [QubitId; 2] = [b.alloc_qubit(), b.alloc_qubit()];
        venting::ciadd_dirty_2clean_classical(
            b,
            v,
            &dirty[..n - 2],
            &q_clean2,
            c.as_limbs()[0],
            ovf,
            false,
        );
        b.free(q_clean2[0]);
        b.free(q_clean2[1]);
    } else if direct_const_walks_enabled()
        || std::env::var("KAL_DIRECT_CONST_DOUBLE").ok().as_deref() == Some("1")
    {
        cadd_nbit_const_direct_fast(b, v, c, ovf);
    } else {
        cadd_nbit_const_fast(b, v, c, ovf);
    }
    // Result parity equals the old top bit: even if ovf=0, odd if ovf=1.
    b.cx(v[0], ovf);
    b.free(ovf);
}

/// Fast `v := v/2 mod p`. Explicit reverse of `mod_double_inplace` with
/// measurement-based Cuccaro (not emit_inverse).
pub(crate) fn mod_halve_inplace_fast(b: &mut B, v: &[QubitId], p: U256) {
    mod_halve_inplace_fast_with_dirty(b, v, p, None)
}

/// Variant of `mod_halve_inplace_fast` that optionally borrows `dirty_src`
/// qubits for the controlled-sub step, using Gidney's venting
/// `cisub_dirty_2clean_classical`. Saves n transient qubits at the peak
/// when dirty qubits are available from the caller.
pub(crate) fn mod_halve_inplace_fast_with_dirty(
    b: &mut B,
    v: &[QubitId],
    p: U256,
    dirty_src: Option<&[QubitId]>,
) {
    let n = v.len();
    let ovf = b.alloc_qubit();
    debug_assert_eq!(n, 256);
    let c = U256::MAX.wrapping_sub(p).wrapping_add(U256::from(1));
    b.cx(v[0], ovf);
    // If caller provided enough dirty qubits AND c fits in u64 (it does
    // for secp256k1: c = 2^32 + 977), use the venting variant.
    let use_venting = kal_vent_halve_enabled() && dirty_src.map_or(false, |d| d.len() >= n - 2);
    if let Some(w) = double_carry_trunc_window() {
        // Carry-tail-truncated sparse-constant sub (inverse of the truncated
        // double; default OFF; same window so double/halve stay exact inverses).
        csub_nbit_const_direct_trunc_fast(b, v, c, ovf, w);
    } else if use_venting {
        // c as u64 (it fits: c = 0x1000003D1).
        // For n=256, we still need to pass the full 256-bit constant via u64.
        // Since c only has 33 bits, u64 is fine.
        let c_u64: u64 = c.as_limbs()[0] | (c.as_limbs()[1] << 32); // hack for U256
                                                                    // Actually, U256 limbs are u64[4]. Bit 32 of U256 is limbs[0] bit 32.
                                                                    // limbs[0] holds bits 0..64. So just take limbs[0] for bits < 64.
        let c_low = c.as_limbs()[0];
        let dirty = dirty_src.unwrap();
        let dirty_slice = &dirty[..n - 2];
        // We need 2 clean ancilla. Alloc them fresh.
        let q_clean2: [QubitId; 2] = [b.alloc_qubit(), b.alloc_qubit()];
        venting::cisub_dirty_2clean_classical(b, v, dirty_slice, &q_clean2, c_low, ovf);
        b.free(q_clean2[0]);
        b.free(q_clean2[1]);
        let _ = c_u64; // unused, c_low is the right value
    } else if direct_const_walks_enabled()
        || std::env::var("KAL_DIRECT_CONST_HALVE").ok().as_deref() == Some("1")
    {
        csub_nbit_const_direct_fast(b, v, c, ovf);
    } else {
        csub_nbit_const_fast(b, v, c, ovf);
    }
    for i in 0..n - 1 {
        b.swap(v[i], v[i + 1]);
    }
    b.swap(v[n - 1], ovf);
    b.free(ovf);
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

    /// `mod_double_inplace_fast`: `v := (2v) mod p` in place, ancilla clean.
    #[test]
    fn mod_double_is_two_x_mod_p() {
        let p = SECP256K1_P;
        let mut b = B::new();
        let v = b.alloc_qubits(256);
        mod_double_inplace_fast(&mut b, &v, p);
        let nq = b.next_qubit as usize;
        let nb = b.next_bit as usize;
        let inputs: std::collections::HashSet<u64> = v.iter().map(|q| q.0).collect();
        let mut seed = Shake128::default();
        seed.update(b"mod_double");
        let mut xof = seed.finalize_xof();
        let mut sim = Simulator::new(nq, nb, &mut xof);
        let xs: Vec<U256> = (0..64).map(|s| rand_lt_p(s as u64 + 7)).collect();
        for s in 0..64 {
            set256(&mut sim, &v, xs[s], s);
        }
        sim.apply_iter(b.ops.iter());
        assert_eq!(sim.phase, 0, "phase garbage");
        for s in 0..64 {
            let x = xs[s];
            let exp = if x >= p - x { x - (p - x) } else { x + x };
            assert_eq!(get256(&sim, &v, s), exp, "v != 2x mod p, shot {s}");
        }
        for q in 0..nq as u64 {
            if !inputs.contains(&q) {
                assert_eq!(sim.qubit(QubitId(q)), 0, "ancilla q{q} not clean");
            }
        }
    }
}
