use super::*;

pub(crate) fn schoolbook_square_symmetric_lowq_selfhosted(
    b: &mut B,
    x: &[QubitId],
    tmp_ext: &[QubitId],
) {
    schoolbook_square_symmetric_lowq_selfhosted_with_clean_supplement(b, x, tmp_ext, &[]);
}

pub(crate) fn schoolbook_square_symmetric_lowq_selfhosted_with_clean_supplement(
    b: &mut B,
    x: &[QubitId],
    tmp_ext: &[QubitId],
    clean_supplement: &[QubitId],
) {
    let n = x.len();
    debug_assert_eq!(tmp_ext.len(), 2 * n);
    let safe_reuse = square_selfhost_safe_lane_reuse_enabled();
    if safe_reuse {
        assert_qubit_slices_disjoint(&[x, tmp_ext, clean_supplement]);
    }
    let gate_prefix_rows = std::env::var("SQUARE_SELFHOST_GATE_PREFIX_ROWS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(0);
    let row_windows = square_row_windows();
    let row_window_min = square_row_window_min_width();
    let max_seg = square_row_max_seg();
    for i in 0..n {
        let width = if i == n - 1 { 1 } else { n - i + 1 };
        let num_cross = if i + 1 < n { n - i - 1 } else { 0 };
        if max_seg > 0 && i >= gate_prefix_rows && width > max_seg {
            let w = width.div_ceil(max_seg);
            square_row_windowed_apply(b, x, tmp_ext, i, width, w, true);
            continue;
        }
        if max_seg == 0 && row_windows >= 1 && i >= gate_prefix_rows && width >= row_window_min {
            square_row_windowed_apply(b, x, tmp_ext, i, width, row_windows, true);
            continue;
        }
        let row = b.alloc_qubits(width);
        b.cx(x[i], row[0]);
        for k in 0..num_cross {
            b.ccx(x[i], x[i + 1 + k], row[k + 2]);
        }
        let hi = 2 * i + width + 1;
        let slice: Vec<QubitId> = tmp_ext[2 * i..hi].to_vec();
        if i < gate_prefix_rows {
            let pad = b.alloc_qubit();
            let mut row_padded = row.clone();
            row_padded.push(pad);
            let c_in = b.alloc_qubit();
            cuccaro_add(b, &row_padded, &slice, c_in);
            b.free(c_in);
            b.free(pad);
        } else if safe_reuse {
            let need = row.len() - square_selfhost_gate_suffix_carries(row.len());
            let avail = tmp_ext.len() - hi;
            let from_tmp = need.min(avail);
            let from_supplement = (need - from_tmp).min(clean_supplement.len());
            let from_global = need - from_tmp - from_supplement;
            let gpool = b.alloc_qubits(from_global);
            let mut carries: Vec<QubitId> = tmp_ext[hi..hi + from_tmp].to_vec();
            carries.extend_from_slice(&clean_supplement[..from_supplement]);
            carries.extend_from_slice(&gpool);
            cuccaro_add_fast_low_to_ext_borrowed_carries_no_cin(b, &row, &slice, &carries);
            b.free_vec(&gpool);
        } else {
            let pad = b.alloc_qubit();
            let mut row_padded = row.clone();
            row_padded.push(pad);
            let c_in = b.alloc_qubit();
            let need = row_padded.len() - 1;
            let avail = tmp_ext.len() - hi;
            let from_tmp = need.min(avail);
            let from_global = need - from_tmp;
            let gpool = b.alloc_qubits(from_global);
            let mut carries: Vec<QubitId> = tmp_ext[hi..hi + from_tmp].to_vec();
            carries.extend_from_slice(&gpool);
            cuccaro_add_fast_borrowed_carries(b, &row_padded, &slice, c_in, &carries);
            b.free(c_in);
            b.free_vec(&gpool);
            b.free(pad);
        }
        b.cx(x[i], row[0]);
        for k in 0..num_cross {
            let m = b.alloc_bit();
            b.hmr(row[k + 2], m);
            b.cz_if(x[i], x[i + 1 + k], m);
        }
        b.free_vec(&row);
    }
}

pub(crate) fn schoolbook_square_symmetric_lowq_selfhosted_inverse(
    b: &mut B,
    x: &[QubitId],
    tmp_ext: &[QubitId],
) {
    schoolbook_square_symmetric_lowq_selfhosted_inverse_with_clean_supplement(b, x, tmp_ext, &[]);
}

pub(crate) fn schoolbook_square_symmetric_lowq_selfhosted_inverse_with_clean_supplement(
    b: &mut B,
    x: &[QubitId],
    tmp_ext: &[QubitId],
    clean_supplement: &[QubitId],
) {
    let n = x.len();
    debug_assert_eq!(tmp_ext.len(), 2 * n);
    let safe_reuse = square_selfhost_safe_lane_reuse_enabled();
    if safe_reuse {
        assert_qubit_slices_disjoint(&[x, tmp_ext, clean_supplement]);
    }
    let gate_prefix_rows = std::env::var("SQUARE_SELFHOST_GATE_PREFIX_ROWS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(0);
    let row_windows = square_row_windows();
    let row_window_min = square_row_window_min_width();
    let max_seg = square_row_max_seg();
    for i in (0..n).rev() {
        let width = if i == n - 1 { 1 } else { n - i + 1 };
        let num_cross = if i + 1 < n { n - i - 1 } else { 0 };
        if max_seg > 0 && i >= gate_prefix_rows && width > max_seg {
            let w = width.div_ceil(max_seg);
            square_row_windowed_apply(b, x, tmp_ext, i, width, w, false);
            continue;
        }
        if max_seg == 0 && row_windows >= 1 && i >= gate_prefix_rows && width >= row_window_min {
            square_row_windowed_apply(b, x, tmp_ext, i, width, row_windows, false);
            continue;
        }
        let row = b.alloc_qubits(width);
        b.cx(x[i], row[0]);
        for k in 0..num_cross {
            b.ccx(x[i], x[i + 1 + k], row[k + 2]);
        }
        let hi = 2 * i + width + 1;
        let slice: Vec<QubitId> = tmp_ext[2 * i..hi].to_vec();
        if i < gate_prefix_rows {
            let pad = b.alloc_qubit();
            let mut row_padded = row.clone();
            row_padded.push(pad);
            let c_in = b.alloc_qubit();
            cuccaro_sub(b, &row_padded, &slice, c_in);
            b.free(c_in);
            b.free(pad);
        } else if safe_reuse {
            let need = row.len() - square_selfhost_gate_suffix_carries(row.len());
            let avail = tmp_ext.len() - hi;
            let from_tmp = need.min(avail);
            let from_supplement = (need - from_tmp).min(clean_supplement.len());
            let from_global = need - from_tmp - from_supplement;
            let gpool = b.alloc_qubits(from_global);
            let mut carries: Vec<QubitId> = tmp_ext[hi..hi + from_tmp].to_vec();
            carries.extend_from_slice(&clean_supplement[..from_supplement]);
            carries.extend_from_slice(&gpool);
            cuccaro_sub_fast_low_to_ext_borrowed_carries_no_cin(b, &row, &slice, &carries);
            b.free_vec(&gpool);
        } else {
            let pad = b.alloc_qubit();
            let mut row_padded = row.clone();
            row_padded.push(pad);
            let c_in = b.alloc_qubit();
            let need = row_padded.len() - 1;
            let avail = tmp_ext.len() - hi;
            let from_tmp = need.min(avail);
            let from_global = need - from_tmp;
            let gpool = b.alloc_qubits(from_global);
            let mut carries: Vec<QubitId> = tmp_ext[hi..hi + from_tmp].to_vec();
            carries.extend_from_slice(&gpool);
            cuccaro_sub_fast_borrowed_carries(b, &row_padded, &slice, c_in, &carries);
            b.free(c_in);
            b.free_vec(&gpool);
            b.free(pad);
        }
        b.cx(x[i], row[0]);
        for k in 0..num_cross {
            let m = b.alloc_bit();
            b.hmr(row[k + 2], m);
            b.cz_if(x[i], x[i + 1 + k], m);
        }
        b.free_vec(&row);
    }
}

#[allow(dead_code)] // retained reference/alternative impl; not on active build path
struct Round84FoldStep {
    shift: usize,
    add: bool,
    wrap: QubitId,
}

#[allow(dead_code)] // retained reference/alternative impl; not on active build path
struct Round84AggregateFold {
    steps: Vec<Round84FoldStep>,
    quotient: Vec<QubitId>,
    correction_wrap: QubitId,
    correction_wrap_owned: bool,
    product: Option<Vec<QubitId>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::Simulator;
    use sha3::{
        digest::{ExtendableOutput, Update, XofReader},
        Shake128,
    };

    fn get<R: XofReader>(sim: &Simulator<'_, R>, qs: &[QubitId], shot: usize) -> u64 {
        let mut v = 0u64;
        for (i, &q) in qs.iter().enumerate() {
            v |= ((sim.qubit(q) >> shot) & 1) << i;
        }
        v
    }

    /// The selfhosted low-qubit square writes `x²` into a zero `tmp_ext`, leaving
    /// `x` unchanged, and its inverse unwrites it back to |0>. Exhaustive over all
    /// n=4 inputs, with every internal ancilla returned to |0> and phase +1.
    #[test]
    fn selfhosted_square_forward_computes_x_squared_and_inverse_uncomputes() {
        const N: usize = 4;
        let mut b = B::new();
        let x = b.alloc_qubits(N);
        let tmp = b.alloc_qubits(2 * N);
        schoolbook_square_symmetric_lowq_selfhosted(&mut b, &x, &tmp);
        let fwd_len = b.ops.len();
        schoolbook_square_symmetric_lowq_selfhosted_inverse(&mut b, &x, &tmp);
        let nq = b.next_qubit as usize;
        let nb = b.next_bit as usize;
        let inputs: std::collections::HashSet<u64> =
            x.iter().chain(tmp.iter()).map(|q| q.0).collect();

        let load = |sim: &mut Simulator<'_, _>| {
            for shot in 0..(1 << N) {
                for (i, &q) in x.iter().enumerate() {
                    if (shot >> i) & 1 != 0 {
                        *sim.qubit_mut(q) |= 1u64 << shot;
                    }
                }
            }
        };

        // (a) forward only: tmp == x², x unchanged, phase clean.
        let mut s1 = Shake128::default();
        s1.update(b"selfhost-sq-fwd-n4");
        let mut xof1 = s1.finalize_xof();
        let mut sim = Simulator::new(nq, nb, &mut xof1);
        load(&mut sim);
        sim.apply_iter(b.ops[..fwd_len].iter());
        assert_eq!(sim.phase, 0, "forward phase garbage");
        for shot in 0..(1 << N) {
            let xv = shot as u64;
            assert_eq!(get(&sim, &tmp, shot), xv * xv, "x={xv}: tmp != x^2");
            assert_eq!(get(&sim, &x, shot), xv, "x={xv} changed by forward");
        }

        // (b) forward + inverse: identity — tmp back to |0>, all ancilla clean.
        let mut s2 = Shake128::default();
        s2.update(b"selfhost-sq-rt-n4");
        let mut xof2 = s2.finalize_xof();
        let mut sim2 = Simulator::new(nq, nb, &mut xof2);
        load(&mut sim2);
        sim2.apply_iter(b.ops.iter());
        assert_eq!(sim2.phase, 0, "roundtrip phase garbage");
        for shot in 0..(1 << N) {
            assert_eq!(get(&sim2, &tmp, shot), 0, "tmp not uncomputed by inverse");
            assert_eq!(get(&sim2, &x, shot), shot as u64, "x changed by roundtrip");
        }
        for q in 0..nq as u64 {
            if !inputs.contains(&q) {
                assert_eq!(sim2.qubit(QubitId(q)), 0, "ancilla q{q} not clean");
            }
        }
    }
}
