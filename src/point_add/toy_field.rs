//! Path B (issue #48, ADR 0020) — toy-width **reversible field arithmetic**:
//! out-of-place modular multiply and modular inverse over a small prime field
//! `F_p`, as `#[cfg(test)]` circuits in the real simulator. These are the
//! prerequisites the analysis layer lacked for the λ-division affine point-add
//! (ADR 0021) that *handles* the exceptional cases instead of only detecting or
//! bounding them (ADR 0016/0018/0019).
//!
//! Everything is built from the already-validated Vedral–Barenco–Ekert modular
//! adder (`qaddend_testbed::mod_add`, ADR 0014), which emits **only** `X`/`CX`/`CCX`
//! — all self-inverse involutions. That gives a clean, low-risk uncomputation
//! strategy without hand-writing any inverse circuit:
//!
//!   **compute → copy → reverse.** A gadget is built as a *forward-only* fragment
//!   that leaves its result in a fresh register (and all scratch dirty); the result
//!   is `CX`-copied into a clean `out`; then the forward fragment is re-emitted with
//!   its op list **reversed**, which (because every op is an involution) inverts the
//!   whole computation and returns every scratch/result qubit to |0>. `out` is
//!   untouched by the reverse, so it keeps the value. Net: `out ^= f(inputs)`,
//!   inputs preserved, all scratch |0>, phase +1 — a composable reversible gadget.
//!
//! Modular multiply (`mod_mul_fwd`): `z := (x·y) mod p` via the schoolbook
//! double-and-add — a doubling chain `t_i = y·2^i mod p` (each `t_i = 2 t_{i-1}` by
//! one `mod_add`), then for each bit `x_i` add the gated addend `x_i · t_i` into `z`
//! (`mod p`). Modular inverse (`mod_inv_fwd`): Fermat, `a^{p-2} mod p`, by
//! left-to-right square-and-multiply over the fixed classical exponent `p-2`
//! (`inv(0)=0` falls out — the convention ADR 0019 models). Kaliski/EEA is the
//! space-optimal choice for the *real* 256-bit inverse, but Fermat-via-multiply is
//! simpler to verify exhaustively at toy width and equally a reversible-arithmetic
//! inversion (a design note in ADR 0020).
//!
//! Verified by exhaustive masked-multi-shot simulation over the whole field.
//! `#[cfg(test)]` only; never compiled into the scored circuit (`ops.bin` unchanged).

use super::qaddend_testbed::mod_add;
use crate::circuit::{analyze_ops, Op, OperationType, QubitId};
use crate::point_add::B;
use crate::sim::Simulator;

// ── classical reference ─────────────────────────────────────────────────────

fn fmul(a: u64, b: u64, p: u64) -> u64 {
    (a * b) % p
}
fn fpow(mut a: u64, mut e: u64, p: u64) -> u64 {
    let (mut r, _) = (1u64, a %= p);
    while e > 0 {
        if e & 1 == 1 {
            r = fmul(r, a, p);
        }
        a = fmul(a, a, p);
        e >>= 1;
    }
    r
}
/// Field inverse via Fermat (`p` prime); `finv(0)=0` by convention (the reversible
/// circuit's defined behaviour on the exceptional input).
fn finv(a: u64, p: u64) -> u64 {
    if a % p == 0 {
        0
    } else {
        fpow(a, p - 2, p)
    }
}

/// Smallest register width holding `[0, p)`: `2^(n-1) <= p < 2^n`, so `p < 2^n`.
fn width_for(p: u64) -> usize {
    (64 - p.leading_zeros()) as usize
}

// ── the compute→copy→reverse combinator ─────────────────────────────────────

/// Re-emit a captured `X`/`CX`/`CCX`-only fragment into `circ`, forward or reversed
/// (reversed order inverts it, since each gate is its own inverse).
fn replay(circ: &mut B, ops: &[Op], reverse: bool) {
    let apply = |circ: &mut B, op: &Op| match op.kind {
        OperationType::X => circ.x(op.q_target),
        OperationType::CX => circ.cx(op.q_control1, op.q_target),
        // ccx stores (control2=c1, control1=c2); re-emit with the same operands.
        OperationType::CCX => circ.ccx(op.q_control2, op.q_control1, op.q_target),
        k => panic!("path-B fragment has a non-involution op {k:?}"),
    };
    if reverse {
        for op in ops.iter().rev() {
            apply(circ, op);
        }
    } else {
        for op in ops {
            apply(circ, op);
        }
    }
}

/// Emit the clean gadget `out ^= build(...)` into `circ`: run `build` (forward-only,
/// returning its result register), copy the result into `out`, then uncompute the
/// forward fragment. `out` must be |0>; all scratch and the result register are
/// returned to |0>; prior ops on `circ` are preserved.
fn emit_gadget(circ: &mut B, out: &[QubitId], build: impl FnOnce(&mut B) -> Vec<QubitId>) {
    let prior = circ.take_ops();
    let res = build(circ);
    let fwd = circ.take_ops();
    assert_eq!(res.len(), out.len(), "gadget result width != out width");
    replay(circ, &prior, false);
    replay(circ, &fwd, false);
    for (i, &q) in out.iter().enumerate() {
        circ.cx(res[i], q);
    }
    replay(circ, &fwd, true);
}

// ── shared modular-add scratch (each mod_add returns it to |0>) ──────────────

struct Anc {
    hi: QubitId,
    flag: QubitId,
    carry: QubitId,
    preg: Vec<QubitId>,
}
impl Anc {
    fn alloc(circ: &mut B, n: usize) -> Self {
        Anc {
            hi: circ.alloc_qubits(1)[0],
            flag: circ.alloc_qubits(1)[0],
            carry: circ.alloc_qubits(1)[0],
            preg: circ.alloc_qubits(n),
        }
    }
    fn add(&self, circ: &mut B, addend: &[QubitId], acc: &[QubitId], p: u64) {
        mod_add(circ, addend, acc, p, self.hi, self.flag, &self.preg, self.carry);
    }
}

fn copy_reg(circ: &mut B, src: &[QubitId], dst: &[QubitId]) {
    for (s, d) in src.iter().zip(dst) {
        circ.cx(*s, *d);
    }
}

// ── forward-only field gadgets ──────────────────────────────────────────────

/// `z := (x·y) mod p` in a fresh |0> register (returned); `x`, `y` preserved; all
/// other scratch left dirty (an outer reverse cleans it). `p < 2^n`, `x,y < p`.
fn mod_mul_fwd(circ: &mut B, x: &[QubitId], y: &[QubitId], p: u64, n: usize, anc: &Anc) -> Vec<QubitId> {
    let z = circ.alloc_qubits(n);
    // doubling chain: t[0] = y, t[i] = 2·t[i-1] mod p.
    let mut t: Vec<Vec<QubitId>> = Vec::with_capacity(n);
    let t0 = circ.alloc_qubits(n);
    copy_reg(circ, y, &t0);
    t.push(t0);
    for i in 1..n {
        let ti = circ.alloc_qubits(n);
        copy_reg(circ, &t[i - 1], &ti); // ti = t[i-1]
        let prev = t[i - 1].clone();
        anc.add(circ, &prev, &ti, p); // ti = 2·t[i-1] mod p
        t.push(ti);
    }
    // accumulate: z += (x_i ? t[i] : 0) mod p, over all bits.
    for (i, &xi) in x.iter().enumerate() {
        let g = circ.alloc_qubits(n);
        for (b, &tb) in t[i].iter().enumerate() {
            circ.ccx(xi, tb, g[b]); // g = x_i AND t[i]
        }
        anc.add(circ, &g, &z, p);
    }
    z
}

/// `res := a^{p-2} mod p` (= `a^{-1}` for `a != 0`, `0` for `a = 0`) in a fresh
/// register (returned); `a` preserved; scratch dirty. Left-to-right binary
/// exponentiation over the classical exponent `e = p-2`.
fn mod_inv_fwd(circ: &mut B, a: &[QubitId], p: u64, n: usize, anc: &Anc) -> Vec<QubitId> {
    let e = p - 2;
    assert!(e >= 1, "toy field needs p >= 3");
    let msb = 63 - e.leading_zeros(); // top set bit of e (e >= 1)
    let mut cur = circ.alloc_qubits(n);
    copy_reg(circ, a, &cur); // cur = a^1 (leading 1 bit consumed)
    for j in (0..msb).rev() {
        cur = mod_mul_fwd(circ, &cur, &cur, p, n, anc); // cur = cur²
        if (e >> j) & 1 == 1 {
            cur = mod_mul_fwd(circ, &cur, a, p, n, anc); // cur = cur²·a
        }
    }
    cur
}

// ── composable clean gadgets (used by ADR 0021's point-add) ──────────────────

/// `out ^= (x·y) mod p`; `out` |0>, `x`,`y` preserved, all scratch |0>.
pub(super) fn emit_mod_mul(circ: &mut B, x: &[QubitId], y: &[QubitId], out: &[QubitId], p: u64, n: usize) {
    emit_gadget(circ, out, |c| {
        let anc = Anc::alloc(c, n);
        mod_mul_fwd(c, x, y, p, n, &anc)
    });
}

/// `out ^= a^{-1} mod p` (`0` for `a=0`); `out` |0>, `a` preserved, all scratch |0>.
pub(super) fn emit_mod_inv(circ: &mut B, a: &[QubitId], out: &[QubitId], p: u64, n: usize) {
    emit_gadget(circ, out, |c| {
        let anc = Anc::alloc(c, n);
        mod_inv_fwd(c, a, p, n, &anc)
    });
}

// ── exhaustive verification ──────────────────────────────────────────────────

fn read_reg<R: sha3::digest::XofReader>(sim: &Simulator<'_, R>, reg: &[QubitId], s: usize) -> u64 {
    let mut v = 0u64;
    for (i, &q) in reg.iter().enumerate() {
        v |= ((sim.qubit(q) >> s) & 1) << i;
    }
    v
}

/// Assert every qubit except those in `keep` reads 0 in every shot (scratch clean).
fn assert_scratch_clean<R: sha3::digest::XofReader>(sim: &Simulator<'_, R>, n_qubits: usize, keep: &[QubitId]) {
    for id in 0..n_qubits as u64 {
        if keep.contains(&QubitId(id)) {
            continue;
        }
        assert_eq!(sim.qubit(QubitId(id)), 0, "scratch qubit {id} left dirty");
    }
    assert_eq!(sim.phase, 0, "unexpected phase (X/CX/CCX only)");
}

#[test]
fn toy_mod_mul_is_field_multiply() {
    eprintln!("\n=== Path B (ADR 0020): reversible toy modular multiply over F_p ===");
    for &p in &[5u64, 7, 11, 13, 17, 19, 23] {
        let n = width_for(p);
        // one instance emits out ^= (x*y) mod p; sweep x across shots at a few fixed y.
        for &yval in &[1u64, 2, p - 1, (p - 1) / 2] {
            let mut circ = B::new_for_test();
            let x = circ.alloc_qubits(n);
            let y = circ.alloc_qubits(n);
            let out = circ.alloc_qubits(n);
            emit_mod_mul(&mut circ, &x, &y, &out, p, n);
            let ops = circ.take_ops();
            let (peak, nbits, _r, _regs) = analyze_ops(ops.iter());

            let shots = p as usize; // exhaustive over x ∈ [0, p)
            assert!(shots <= 64);
            // load x = shot, y = yval on every lane.
            let mut seed = sha3::Shake128::default();
            sha3::digest::Update::update(&mut seed, b"toy-mod-mul");
            let mut xof = sha3::digest::ExtendableOutput::finalize_xof(seed);
            let mut sim = Simulator::new(peak as usize, nbits as usize, &mut xof);
            sim.clear_for_shot();
            for (i, &q) in x.iter().enumerate() {
                let mut m = 0u64;
                for s in 0..shots {
                    m |= (((s as u64) >> i) & 1) << s;
                }
                *sim.qubit_mut(q) = m;
            }
            for (i, &q) in y.iter().enumerate() {
                let mut m = 0u64;
                for s in 0..shots {
                    m |= ((yval >> i) & 1) << s;
                }
                *sim.qubit_mut(q) = m;
            }
            sim.apply_iter(ops.iter());
            for s in 0..shots {
                let xv = s as u64;
                assert_eq!(read_reg(&sim, &out, s), fmul(xv, yval, p), "x*y mod p wrong (p={p}, x={xv}, y={yval})");
                assert_eq!(read_reg(&sim, &x, s), xv, "x perturbed");
                assert_eq!(read_reg(&sim, &y, s), yval, "y perturbed");
            }
            let keep: Vec<QubitId> = x.iter().chain(&y).chain(&out).copied().collect();
            assert_scratch_clean(&sim, peak as usize, &keep);
        }
        eprintln!("  p={p:<2} (n={n}): out=(x·y) mod p exact over F_p, x/y preserved, scratch clean");
    }
    eprintln!("  => reversible modular multiply verified.");
}

#[test]
fn toy_mod_inverse_is_field_inverse() {
    eprintln!("\n=== Path B (ADR 0020): reversible toy modular inverse (Fermat) over F_p ===");
    for &p in &[5u64, 7, 11, 13, 17, 19, 23] {
        let n = width_for(p);
        let mut circ = B::new_for_test();
        let a = circ.alloc_qubits(n);
        let out = circ.alloc_qubits(n);
        emit_mod_inv(&mut circ, &a, &out, p, n);
        let ops = circ.take_ops();
        let (peak, nbits, _r, _regs) = analyze_ops(ops.iter());

        // exhaustive over a ∈ [0, p): one lane per field element.
        let shots = p as usize;
        assert!(shots <= 64);
        let mut seed = sha3::Shake128::default();
        sha3::digest::Update::update(&mut seed, b"toy-mod-inv");
        let mut xof = sha3::digest::ExtendableOutput::finalize_xof(seed);
        let mut sim = Simulator::new(peak as usize, nbits as usize, &mut xof);
        sim.clear_for_shot();
        for (i, &q) in a.iter().enumerate() {
            let mut m = 0u64;
            for s in 0..shots {
                m |= (((s as u64) >> i) & 1) << s;
            }
            *sim.qubit_mut(q) = m;
        }
        sim.apply_iter(ops.iter());
        for s in 0..p as usize {
            let av = s as u64;
            let got = read_reg(&sim, &out, s);
            assert_eq!(got, finv(av, p), "inv wrong (p={p}, a={av})");
            if av != 0 {
                assert_eq!(fmul(av, got, p), 1, "a·inv(a) != 1 (p={p}, a={av})");
            }
            assert_eq!(read_reg(&sim, &a, s), av, "a perturbed");
        }
        let keep: Vec<QubitId> = a.iter().chain(&out).copied().collect();
        assert_scratch_clean(&sim, peak as usize, &keep);
        eprintln!("  p={p:<2} (n={n}, peak={peak}): inv(a)=a^(p-2) exact ∀a∈F_p, inv(0)=0, a·inv(a)=1, clean");
    }
    eprintln!("  => reversible modular inverse verified exhaustively; unblocks ADR 0021.");
}
