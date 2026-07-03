//! Path B (issue #48, ADR 0021) — a reversible **λ-division affine point-add** at
//! toy width that *HANDLES* the exceptional cases (doubling, `P = −Q → ∞`, `∞`
//! operands), instead of only detecting them (ADR 0018) or bounding their amplitude
//! (ADR 0016) or demonstrating recovery despite the incomplete adder's misfires
//! (ADR 0019). This is the deferred Path-B increment: the affine adder made
//! **complete** on a real toy curve, as a `#[cfg(test)]` circuit in the simulator.
//!
//! It composes the reversible field arithmetic of ADR 0020 (`toy_field`): modular
//! add/sub (`Anc::add`/`sub`), multiply (`mod_mul_fwd`) and inverse (`mod_inv_fwd`).
//! The whole point-add is one *compute → copy → reverse* gadget (every op is
//! `X`/`CX`/`CCX`, so re-emitting the forward fragment reversed uncomputes all
//! scratch): the forward pass computes the slope `λ` (chord `(y₂−y₁)/(x₂−x₁)` or,
//! when `P = Q`, the tangent `(3x₁²+a)/(2y₁)`), the affine result
//! `x₃ = λ²−x₁−x₂`, `y₃ = λ(x₁−x₃)−y₁`, and the exceptional-case flags (`P=∞`,
//! `Q=∞`, `P=−Q`) using the ADR 0018 zero-tests; a mutually-exclusive **mux** then
//! copies the correct result into the clean output — `Q` if `P=∞`, `P` if `Q=∞`,
//! `∞ = (0,0)` if `P=−Q`, else `(x₃,y₃)` — and the reverse cleans everything.
//!
//! The generic branch's denominator is never 0 (chord: `x₁≠x₂`; tangent: `2y₁≠0` on
//! a prime-order curve), and `inv(0)=0` keeps the overridden ∞/neg branches from
//! dividing by zero. Verified **exhaustively** over **every** `(P, Q)` pair —
//! including all exceptional pairs — of real prime-order toy curves (orders
//! 19/29/41): the reversible output equals the reference group law, inputs
//! preserved, all scratch |0>, phase +1.
//!
//! `#[cfg(test)]` only; never compiled into the scored circuit (`ops.bin` unchanged).

use super::toy_field::{copy_reg, finv, mod_inv_fwd, mod_mul_fwd, replay, width_for, Anc};
use crate::circuit::{analyze_ops, Op, QubitId};
use crate::point_add::B;
use crate::sim::Simulator;

// ── classical reference (complete group law) ─────────────────────────────────

fn is_prime(x: u64) -> bool {
    x >= 2
        && (2..x)
            .take_while(|d| d * d <= x)
            .all(|d| !x.is_multiple_of(d))
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Pt {
    Inf,
    Aff(u64, u64),
}

fn ec_add(pa: Pt, qb: Pt, a: u64, p: u64) -> Pt {
    use super::toy_field::{fadd, fmul, fsub};
    match (pa, qb) {
        (Pt::Inf, q) => q,
        (pp, Pt::Inf) => pp,
        (Pt::Aff(x1, y1), Pt::Aff(x2, y2)) => {
            if x1 == x2 && (y1 + y2) % p == 0 {
                return Pt::Inf; // P + (−P) = ∞
            }
            let lam = if x1 == x2 && y1 == y2 {
                fmul(
                    fadd(fmul(3, fmul(x1, x1, p), p), a, p),
                    finv(fmul(2, y1, p), p),
                    p,
                )
            } else {
                fmul(fsub(y2, y1, p), finv(fsub(x2, x1, p), p), p)
            };
            let x3 = fsub(fsub(fmul(lam, lam, p), x1, p), x2, p);
            let y3 = fsub(fmul(lam, fsub(x1, x3, p), p), y1, p);
            Pt::Aff(x3, y3)
        }
    }
}

fn ec_mul(mut k: u64, g: Pt, a: u64, p: u64) -> Pt {
    let (mut acc, mut base) = (Pt::Inf, g);
    while k > 0 {
        if k & 1 == 1 {
            acc = ec_add(acc, base, a, p);
        }
        base = ec_add(base, base, a, p);
        k >>= 1;
    }
    acc
}

/// `(0,0)` is the `∞` sentinel — required off-curve so it never aliases a real point.
fn coords(pt: Pt) -> (u64, u64) {
    match pt {
        Pt::Inf => (0, 0),
        Pt::Aff(x, y) => (x, y),
    }
}

/// A real prime-order toy curve `y² = x³ + ax + b / F_p` and its points (incl. ∞).
struct ToyCurve {
    points: Vec<Pt>,
}
impl ToyCurve {
    fn new(p: u64, a: u64, b: u64) -> Self {
        use super::toy_field::{fadd, fmul};
        let on = |x: u64, y: u64| {
            fmul(y, y, p) == fadd(fadd(fmul(x, fmul(x, x, p), p), fmul(a, x, p), p), b, p)
        };
        assert!(is_prime(p), "field p={p} must be prime (Fermat inverse)");
        assert!(
            !on(0, 0),
            "(0,0) ∞-sentinel must be off-curve (p={p},a={a},b={b})"
        );
        let mut pts = vec![Pt::Inf];
        for x in 0..p {
            for y in 0..p {
                if on(x, y) {
                    pts.push(Pt::Aff(x, y));
                }
            }
        }
        let order = pts.len() as u64;
        assert!(
            is_prime(order),
            "want a prime-order curve, got order {order} (p={p},a={a},b={b})"
        );
        // prime order ⇒ every non-identity point generates; take the first as a sanity generator.
        Self { points: pts }
    }
}

// ── reversible primitives ────────────────────────────────────────────────────

/// `flag ^= (reg == 0)`. Forward-only (dirty AND-spine, cleaned by the outer reverse).
fn is_zero_fwd(circ: &mut B, reg: &[QubitId], flag: QubitId) {
    for &q in reg {
        circ.x(q); // bit = 1 iff original bit was 0
    }
    if reg.len() == 1 {
        circ.cx(reg[0], flag);
    } else {
        let spine = circ.alloc_qubits(reg.len() - 1);
        circ.ccx(reg[0], reg[1], spine[0]);
        for i in 2..reg.len() {
            circ.ccx(spine[i - 2], reg[i], spine[i - 1]);
        }
        circ.cx(spine[reg.len() - 2], flag); // flag ^= AND(all flipped bits) = (reg==0)
    }
    for &q in reg {
        circ.x(q); // restore
    }
}

/// Load the classical constant `val` into a |0> register (self-inverse; a second call clears it).
fn load_const(circ: &mut B, reg: &[QubitId], val: u64) {
    for (i, &q) in reg.iter().enumerate() {
        if (val >> i) & 1 == 1 {
            circ.x(q);
        }
    }
}

/// `dst[b] ^= ctrl AND src[b]` for every bit — a controlled register copy.
fn ctrl_copy(circ: &mut B, ctrl: QubitId, src: &[QubitId], dst: &[QubitId]) {
    assert_eq!(src.len(), dst.len(), "ctrl_copy width mismatch");
    for (&s, &d) in src.iter().zip(dst) {
        circ.ccx(ctrl, s, d);
    }
}

/// Forward pass: compute the affine result `(x3,y3)` and the exclusive mux controls
/// for the ∞/neg cases. Returns `(c_pinf, c_qinf, c_gen, x3, y3)`; all inputs preserved.
#[allow(clippy::too_many_arguments)]
fn pointadd_forward(
    circ: &mut B,
    x1: &[QubitId],
    y1: &[QubitId],
    x2: &[QubitId],
    y2: &[QubitId],
    a: u64,
    p: u64,
    n: usize,
    anc: &Anc,
) -> (QubitId, QubitId, QubitId, Vec<QubitId>, Vec<QubitId>) {
    let a1 = |c: &mut B| c.alloc_qubits(1)[0];

    // ∞ flags: P=∞ ⇔ (x1,y1)=(0,0); Q=∞ ⇔ (x2,y2)=(0,0).
    let (tx1, ty1) = (a1(circ), a1(circ));
    is_zero_fwd(circ, x1, tx1);
    is_zero_fwd(circ, y1, ty1);
    let p_inf = a1(circ);
    circ.ccx(tx1, ty1, p_inf);
    let (tx2, ty2) = (a1(circ), a1(circ));
    is_zero_fwd(circ, x2, tx2);
    is_zero_fwd(circ, y2, ty2);
    let q_inf = a1(circ);
    circ.ccx(tx2, ty2, q_inf);

    // dx = x2−x1, dy = y2−y1, sy = y1+y2.
    let dx = circ.alloc_qubits(n);
    copy_reg(circ, x2, &dx);
    anc.sub(circ, x1, &dx, p);
    let dy = circ.alloc_qubits(n);
    copy_reg(circ, y2, &dy);
    anc.sub(circ, y1, &dy, p);
    let sy = circ.alloc_qubits(n);
    copy_reg(circ, y1, &sy);
    anc.add(circ, y2, &sy, p);

    let eqx = a1(circ);
    is_zero_fwd(circ, &dx, eqx); // x1==x2
    let dyz = a1(circ);
    is_zero_fwd(circ, &dy, dyz); // y1==y2
    let syz = a1(circ);
    is_zero_fwd(circ, &sy, syz); // y1==−y2
    let dbl = a1(circ);
    circ.ccx(eqx, dyz, dbl); // P==Q
    let neg = a1(circ);
    circ.ccx(eqx, syz, neg); // P==−Q → ∞

    // slope numerator/denominator: chord (dy/dx), plus the doubling (3x1²+a)/(2y1)
    // gated on `dbl`. For a true double, dx=dy=0, so adding the gated tangent terms
    // into num/den yields exactly the tangent; the chord case is untouched.
    let num = circ.alloc_qubits(n);
    copy_reg(circ, &dy, &num);
    let den = circ.alloc_qubits(n);
    copy_reg(circ, &dx, &den);
    let x1sq = mod_mul_fwd(circ, x1, x1, p, n, anc);
    let tri = circ.alloc_qubits(n);
    anc.add(circ, &x1sq, &tri, p);
    anc.add(circ, &x1sq, &tri, p);
    anc.add(circ, &x1sq, &tri, p); // 3·x1²
    let areg = circ.alloc_qubits(n);
    load_const(circ, &areg, a);
    anc.add(circ, &areg, &tri, p); // +a
    load_const(circ, &areg, a); // unload a
    let twoy = circ.alloc_qubits(n);
    anc.add(circ, y1, &twoy, p);
    anc.add(circ, y1, &twoy, p); // 2·y1
    let gtri = circ.alloc_qubits(n);
    ctrl_copy(circ, dbl, &tri, &gtri);
    anc.add(circ, &gtri, &num, p);
    let gtwoy = circ.alloc_qubits(n);
    ctrl_copy(circ, dbl, &twoy, &gtwoy);
    anc.add(circ, &gtwoy, &den, p);

    // λ = num · inv(den); (x3,y3).
    let invden = mod_inv_fwd(circ, &den, p, n, anc);
    let lam = mod_mul_fwd(circ, &num, &invden, p, n, anc);
    let lamsq = mod_mul_fwd(circ, &lam, &lam, p, n, anc);
    let x3 = circ.alloc_qubits(n);
    copy_reg(circ, &lamsq, &x3);
    anc.sub(circ, x1, &x3, p);
    anc.sub(circ, x2, &x3, p); // x3 = λ²−x1−x2
    let x1mx3 = circ.alloc_qubits(n);
    copy_reg(circ, x1, &x1mx3);
    anc.sub(circ, &x3, &x1mx3, p); // x1−x3
    let prod = mod_mul_fwd(circ, &lam, &x1mx3, p, n, anc);
    let y3 = circ.alloc_qubits(n);
    copy_reg(circ, &prod, &y3);
    anc.sub(circ, y1, &y3, p); // y3 = λ(x1−x3)−y1

    // exclusive mux controls: c_pinf = P∞; c_qinf = Q∞ & ¬P∞;
    // c_gen = ¬P∞ & ¬Q∞ & ¬neg (chord/tangent). c_neg (∞ output) copies nothing.
    let c_pinf = p_inf;
    let c_qinf = a1(circ);
    circ.x(p_inf);
    circ.ccx(q_inf, p_inf, c_qinf);
    circ.x(p_inf);
    let notpq = a1(circ);
    circ.x(p_inf);
    circ.x(q_inf);
    circ.ccx(p_inf, q_inf, notpq);
    circ.x(q_inf);
    circ.x(p_inf);
    let c_gen = a1(circ);
    circ.x(neg);
    circ.ccx(notpq, neg, c_gen);
    circ.x(neg);

    (c_pinf, c_qinf, c_gen, x3, y3)
}

/// Emit the complete reversible point-add: `(ox,oy) ^= P + Q` (the group law, all
/// exceptional cases handled). `ox`,`oy` must be |0>; inputs preserved; all scratch
/// returned to |0>.
#[allow(clippy::too_many_arguments)]
fn emit_point_add(
    circ: &mut B,
    x1: &[QubitId],
    y1: &[QubitId],
    x2: &[QubitId],
    y2: &[QubitId],
    ox: &[QubitId],
    oy: &[QubitId],
    a: u64,
    p: u64,
    n: usize,
) {
    let anc = Anc::alloc(circ, n);
    let start = circ.ops.len();
    let (c_pinf, c_qinf, c_gen, x3, y3) = pointadd_forward(circ, x1, y1, x2, y2, a, p, n, &anc);
    let fwd: Vec<Op> = circ.ops[start..].to_vec();
    // mux (copy step): pick Q (P∞), P (Q∞), (x3,y3) (generic); ∞ copies nothing.
    ctrl_copy(circ, c_pinf, x2, ox);
    ctrl_copy(circ, c_qinf, x1, ox);
    ctrl_copy(circ, c_gen, &x3, ox);
    ctrl_copy(circ, c_pinf, y2, oy);
    ctrl_copy(circ, c_qinf, y1, oy);
    ctrl_copy(circ, c_gen, &y3, oy);
    replay(circ, &fwd, true); // uncompute all scratch
}

// ── exhaustive verification over real prime-order toy curves ─────────────────

fn read_reg<R: sha3::digest::XofReader>(sim: &Simulator<'_, R>, reg: &[QubitId], s: usize) -> u64 {
    let mut v = 0u64;
    for (i, &q) in reg.iter().enumerate() {
        v |= ((sim.qubit(q) >> s) & 1) << i;
    }
    v
}

#[test]
fn toy_point_add_handles_all_exceptional_cases() {
    eprintln!("\n=== Path B (ADR 0021): complete reversible λ-division affine point-add ===");
    // (p, a, b): prime field, prime group order (asserted in ToyCurve::new).
    let curves = [(17u64, 2u64, 2u64), (23, 1, 4), (31, 1, 3)];
    for (p, a, b) in curves {
        let curve = ToyCurve::new(p, a, b);
        let n = width_for(p);
        let pts = &curve.points;
        let m = pts.len();
        assert!(m <= 64, "need ≤64 points to sweep Q across shots (got {m})");

        // build the circuit once; re-simulate per fixed P with Q swept over shots.
        let mut circ = B::new_for_test();
        let x1 = circ.alloc_qubits(n);
        let y1 = circ.alloc_qubits(n);
        let x2 = circ.alloc_qubits(n);
        let y2 = circ.alloc_qubits(n);
        let ox = circ.alloc_qubits(n);
        let oy = circ.alloc_qubits(n);
        emit_point_add(&mut circ, &x1, &y1, &x2, &y2, &ox, &oy, a, p, n);
        let ops = circ.take_ops();
        let (peak, nbits, _r, _regs) = analyze_ops(ops.iter());
        let keep: Vec<QubitId> = [&x1, &y1, &x2, &y2, &ox, &oy]
            .iter()
            .flat_map(|r| r.iter().copied())
            .collect();

        let order = m as u64; // group order = point count (incl. ∞); prime by ToyCurve::new
        let mut exceptional = 0usize;
        for &pp in pts {
            let (px, py) = coords(pp);
            let mut seed = sha3::Shake128::default();
            sha3::digest::Update::update(&mut seed, b"toy-point-add");
            let mut xof = sha3::digest::ExtendableOutput::finalize_xof(seed);
            let mut sim = Simulator::new(peak as usize, nbits as usize, &mut xof);
            sim.clear_for_shot();
            // x1,y1 = P on every lane; x2,y2 = pts[s] per lane.
            let load =
                |sim: &mut Simulator<'_, _>, reg: &[QubitId], per_shot: &dyn Fn(usize) -> u64| {
                    for (i, &q) in reg.iter().enumerate() {
                        let mut msk = 0u64;
                        for s in 0..m {
                            msk |= ((per_shot(s) >> i) & 1) << s;
                        }
                        *sim.qubit_mut(q) = msk;
                    }
                };
            load(&mut sim, &x1, &|_| px);
            load(&mut sim, &y1, &|_| py);
            load(&mut sim, &x2, &|s| coords(pts[s]).0);
            load(&mut sim, &y2, &|s| coords(pts[s]).1);
            sim.apply_iter(ops.iter());

            for (s, &q) in pts.iter().enumerate() {
                let (ex, ey) = coords(ec_add(pp, q, a, p));
                assert_eq!(
                    read_reg(&sim, &ox, s),
                    ex,
                    "x3 wrong: {pp:?} + {q:?} (p={p})"
                );
                assert_eq!(
                    read_reg(&sim, &oy, s),
                    ey,
                    "y3 wrong: {pp:?} + {q:?} (p={p})"
                );
                // inputs preserved
                assert_eq!(
                    (read_reg(&sim, &x1, s), read_reg(&sim, &y1, s)),
                    (px, py),
                    "P perturbed"
                );
                assert_eq!(
                    (read_reg(&sim, &x2, s), read_reg(&sim, &y2, s)),
                    coords(q),
                    "Q perturbed"
                );
                if matches!(pp, Pt::Inf)
                    || matches!(q, Pt::Inf)
                    || pp == q
                    || ec_add(pp, q, a, p) == Pt::Inf
                {
                    exceptional += 1;
                }
            }
            // all scratch clean, every shot, this P.
            for id in 0..peak {
                if keep.contains(&QubitId(id)) {
                    continue;
                }
                assert_eq!(
                    sim.qubit(QubitId(id)),
                    0,
                    "scratch qubit {id} dirty (P={pp:?}, p={p})"
                );
            }
            assert_eq!(sim.phase, 0, "unexpected phase");
        }
        // sanity: a generator's order is the prime group order (curve really is what we assert).
        let gen = pts.iter().find(|p| !matches!(p, Pt::Inf)).copied().unwrap();
        assert_eq!(
            ec_mul(order, gen, a, p),
            Pt::Inf,
            "generator order != group order"
        );
        eprintln!(
            "  y²=x³+{a}x+{b} / F_{p}: group order {order} (prime), all {m}×{m} (P,Q) pairs correct \
             incl. {exceptional} exceptional (∞/doubling/P=−Q); inputs preserved, scratch clean"
        );
    }
    eprintln!(
        "  => the affine adder is COMPLETE on real toy curves — Path B handled, not just detected."
    );
}
