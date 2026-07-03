//! Tier B / completeness (issue #28), the remaining piece of #5: the
//! **circuit-level** mid-ladder exceptional confirmation over **real coordinate
//! arithmetic**, complementing the scalar/dlog-model exact bound of ADR 0016
//! (`analysis/verify/mid_ladder_bound.py`).
//!
//! The completeness argument (ADR 0006/0008) and the exact end-to-end bound
//! (ADR 0016) both work in the **scalar/dlog model**: a point is its discrete log
//! `s ∈ Z_n`, `∞` is `s=0`, and `[s]P`, `[t]P` share an x-coordinate iff
//! `t ≡ ±s (mod n)`. That last equivalence is the one *curve* fact the whole bound
//! rests on. #15 cross-checked it against a real curve in Python; what was missing
//! (and is #28's remaining item) is a **circuit-level** confirmation: a reversible
//! detector operating on real `(x, y)` coordinate qubits, over a real prime-order
//! curve, agreeing with the scalar predicate on every accumulator/addend pair.
//!
//! Key simplification: the affine collision `dx = 0` is exactly `x1 == x2` — a
//! **bitwise x-coordinate equality**, needing no modular inverse or the full
//! λ-division point-add. So the exceptional set is detectable by a small reversible
//! circuit (an x-equality test + two `∞`-sentinel zero-tests), which is what a real
//! ladder would use to *detect* (and a completeness proof to *bound*) the bad
//! inputs. This harness:
//!
//!   1. builds a real toy curve `y² = x³ + ax + b` over `F_p`, finds a base point
//!      `G` of **prime** order `n` (asserted), and tabulates `[k]G` for all `k`;
//!   2. builds a reversible detector on the `B` emitter — `dx0 = (x1==x2)`,
//!      `acc_inf = ((x1,y1)==sentinel)`, `add_inf = ((x2,y2)==sentinel)` — and
//!      simulation-verifies it on crafted exceptional/generic inputs (ancilla clean);
//!   3. **measures** the detector over ALL `(accumulator, addend)` coordinate pairs
//!      of the group (masked multi-shot), across **several** prime-order curves
//!      (orders 19/29/41), and asserts the measured real-coordinate exceptional
//!      predicate equals the scalar/dlog predicate `(m==0) ∨ (y==0) ∨ (y ≡ ±m)` on
//!      every pair — the circuit-level confirmation of the model;
//!   4. drives the ADR 0016 end-to-end survival recursion with the CIRCUIT-measured
//!      predicate over the real two-scalar `[a]P+[b]Q` toy ladder, at window widths
//!      `w = 2..5` (not a single-`w` artifact), reporting the exact mid-ladder
//!      residual and confirming `exact ≤ union`, and that the **offset** encoding
//!      (ADR 0015) yields `add_inf = 0` at every window on real coordinates — the
//!      zero-window-`∞` pin, circuit-confirmed.
//!
//! This is Path A (negligibility, ADR 0006): the detector *characterises* the rare
//! exceptional inputs — it does not add complete-formula *handling* to the scored
//! adder (that would be Path B and change the score). The full reversible λ-division
//! point-add is a separate increment.
//!
//! `#[cfg(test)]` only; never compiled into the scored circuit (ops.bin unchanged).

use crate::circuit::QubitId;
use crate::point_add::B;
use crate::sim::Simulator;

// ── classical toy-curve arithmetic over F_p (for tables + cross-check) ───────

fn fadd(a: u64, b: u64, p: u64) -> u64 {
    (a + b) % p
}
fn fsub(a: u64, b: u64, p: u64) -> u64 {
    (a + p - b % p) % p
}
fn fmul(a: u64, b: u64, p: u64) -> u64 {
    (a * b) % p
}
fn fpow(mut a: u64, mut e: u64, p: u64) -> u64 {
    let mut r = 1u64;
    a %= p;
    while e > 0 {
        if e & 1 == 1 {
            r = fmul(r, a, p);
        }
        a = fmul(a, a, p);
        e >>= 1;
    }
    r
}
fn finv(a: u64, p: u64) -> u64 {
    fpow(a, p - 2, p) // p prime
}

/// Affine EC point: `∞` or `(x, y)` on `y² = x³ + ax + b` over `F_p`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Pt {
    Inf,
    Aff(u64, u64),
}

fn ec_add(pa: Pt, qb: Pt, a: u64, p: u64) -> Pt {
    match (pa, qb) {
        (Pt::Inf, q) => q,
        (pp, Pt::Inf) => pp,
        (Pt::Aff(x1, y1), Pt::Aff(x2, y2)) => {
            if x1 == x2 && (y1 + y2) % p == 0 {
                return Pt::Inf; // P + (−P) = ∞
            }
            let lam = if x1 == x2 && y1 == y2 {
                // doubling: (3x1² + a) / (2 y1)
                fmul(
                    fadd(fmul(3, fmul(x1, x1, p), p), a, p),
                    finv(fmul(2, y1, p), p),
                    p,
                )
            } else {
                // chord: (y2 − y1) / (x2 − x1)
                fmul(fsub(y2, y1, p), finv(fsub(x2, x1, p), p), p)
            };
            let x3 = fsub(fsub(fmul(lam, lam, p), x1, p), x2, p);
            let y3 = fsub(fmul(lam, fsub(x1, x3, p), p), y1, p);
            Pt::Aff(x3, y3)
        }
    }
}

fn ec_mul(mut k: u64, g: Pt, a: u64, p: u64) -> Pt {
    let mut acc = Pt::Inf;
    let mut base = g;
    while k > 0 {
        if k & 1 == 1 {
            acc = ec_add(acc, base, a, p);
        }
        base = ec_add(base, base, a, p);
        k >>= 1;
    }
    acc
}

/// The `(0, 0)` `∞` sentinel — off-curve for our params, so it never collides with
/// a real point. Coordinates of a point for the detector (`∞ → (0,0)`).
fn coords(pt: Pt) -> (u64, u64) {
    match pt {
        Pt::Inf => (0, 0),
        Pt::Aff(x, y) => (x, y),
    }
}

/// A prime-order toy curve and its cyclic group `⟨G⟩ ≅ Z_n` tabulated as
/// `table[k] = [k]G` (with `[0]G = ∞`).
struct ToyCurve {
    p: u64,
    a: u64,
    n: u64,
    table: Vec<Pt>,
}

fn is_prime(x: u64) -> bool {
    if x < 2 {
        return false;
    }
    let mut d = 2;
    while d * d <= x {
        if x.is_multiple_of(d) {
            return false;
        }
        d += 1;
    }
    true
}

impl ToyCurve {
    /// `y² = x³ + ax + b` over `F_p`; picks the first generator whose order is the
    /// full group order, requiring that order to be prime (so `⟨G⟩` is the whole
    /// curve and every nonzero point is some `[k]G`). Panics if no prime-order
    /// curve results from the params — a loud failure, not a silent wrong model.
    fn new(p: u64, a: u64, b: u64) -> Self {
        let on_curve = |x: u64, y: u64| {
            fmul(y, y, p) == fadd(fadd(fmul(x, fmul(x, x, p), p), fmul(a, x, p), p), b, p)
        };
        // `finv` uses Fermat inversion (a^(p−2)), valid only for prime `p`; and the
        // `(0,0)` `∞` sentinel must be off-curve so it never aliases a real point.
        // Assert both up front — a param change can't silently invalidate the model.
        assert!(
            is_prime(p),
            "field modulus p={p} must be prime (Fermat inversion)"
        );
        assert!(
            !on_curve(0, 0),
            "(0,0) ∞-sentinel must be off-curve for (p={p},a={a},b={b})"
        );
        let mut pts = vec![Pt::Inf];
        for x in 0..p {
            for y in 0..p {
                if on_curve(x, y) {
                    pts.push(Pt::Aff(x, y));
                }
            }
        }
        let order = pts.len() as u64;
        assert!(
            is_prime(order),
            "toy curve order {order} not prime (pick other params)"
        );
        // Any non-∞ point generates the whole prime-order group.
        let g = *pts
            .iter()
            .find(|q| **q != Pt::Inf)
            .expect("no finite point");
        let table: Vec<Pt> = (0..order).map(|k| ec_mul(k, g, a, p)).collect();
        // Sanity: the tabulated group is exactly Z_n under dlog (bijective).
        assert_eq!(table[0], Pt::Inf);
        assert_eq!(table[1], g);
        Self {
            p,
            a,
            n: order,
            table,
        }
    }

    fn bits(&self) -> usize {
        (64 - (self.p - 1).leading_zeros()) as usize // bits to hold 0..p-1
    }
}

// ── reversible detector on the B emitter ────────────────────────────────────

/// `flag ^= (reg == 0)`, reversibly, with `anc` (len `reg.len()-1`, ≥0) returned
/// to |0>. An AND-tree over the X-complemented register: all-ones ⟺ reg==0.
fn zero_test(circ: &mut B, reg: &[QubitId], flag: QubitId, anc: &[QubitId]) {
    let k = reg.len();
    assert!(k >= 1);
    for &q in reg {
        circ.x(q);
    }
    if k == 1 {
        circ.cx(reg[0], flag);
    } else {
        assert!(anc.len() >= k - 1);
        circ.ccx(reg[0], reg[1], anc[0]);
        for i in 1..k - 1 {
            circ.ccx(anc[i - 1], reg[i + 1], anc[i]);
        }
        circ.cx(anc[k - 2], flag);
        // uncompute the AND-tree
        for i in (1..k - 1).rev() {
            circ.ccx(anc[i - 1], reg[i + 1], anc[i]);
        }
        circ.ccx(reg[0], reg[1], anc[0]);
    }
    for &q in reg {
        circ.x(q);
    }
}

/// Detector registers built on `circ`, over `bits`-wide coordinates.
struct Detector {
    x1: Vec<QubitId>,
    y1: Vec<QubitId>,
    x2: Vec<QubitId>,
    y2: Vec<QubitId>,
    dx0: QubitId,
    acc_inf: QubitId,
    add_inf: QubitId,
    all: usize, // total qubits allocated (for the simulator width)
}

/// Emit the exceptional detector: `dx0 = (x1==x2)`, `acc_inf = ((x1,y1)==0,0)`,
/// `add_inf = ((x2,y2)==0,0)`. Every scratch qubit is returned to |0>; the three
/// flag qubits carry the (basis-diagonal) results. Operates purely on the real
/// coordinate qubits — no modular inverse, no λ-division.
fn build_detector(bits: usize) -> (B, Detector, Vec<QubitId>) {
    let mut circ = B::new_for_test();
    let x1 = circ.alloc_qubits(bits);
    let y1 = circ.alloc_qubits(bits);
    let x2 = circ.alloc_qubits(bits);
    let y2 = circ.alloc_qubits(bits);
    let dx0 = circ.alloc_qubits(1)[0];
    let acc_inf = circ.alloc_qubits(1)[0];
    let add_inf = circ.alloc_qubits(1)[0];
    let xor = circ.alloc_qubits(bits); // tmp = x1 ^ x2 for the equality test
    let anc = circ.alloc_qubits(2 * bits); // AND-tree scratch (≥ 2*bits-1 needed)

    // dx0 = (x1 == x2): tmp = x1 ^ x2, then dx0 ^= (tmp == 0), uncompute tmp.
    for (i, &q) in xor.iter().enumerate() {
        circ.cx(x1[i], q);
        circ.cx(x2[i], q);
    }
    zero_test(&mut circ, &xor, dx0, &anc);
    for (i, &q) in xor.iter().enumerate() {
        circ.cx(x1[i], q);
        circ.cx(x2[i], q);
    }
    // acc_inf = ((x1,y1) == (0,0)); add_inf = ((x2,y2) == (0,0)).
    let acc_reg: Vec<QubitId> = x1.iter().chain(y1.iter()).copied().collect();
    let add_reg: Vec<QubitId> = x2.iter().chain(y2.iter()).copied().collect();
    zero_test(&mut circ, &acc_reg, acc_inf, &anc);
    zero_test(&mut circ, &add_reg, add_inf, &anc);

    let all = circ.next_qubit as usize;
    let det = Detector {
        x1,
        y1,
        x2,
        y2,
        dx0,
        acc_inf,
        add_inf,
        all,
    };
    let inputs: Vec<QubitId> = det
        .x1
        .iter()
        .chain(det.y1.iter())
        .chain(det.x2.iter())
        .chain(det.y2.iter())
        .copied()
        .collect();
    (circ, det, inputs)
}

fn read_reg<R: sha3::digest::XofReader>(sim: &Simulator<'_, R>, reg: &[QubitId], s: usize) -> u64 {
    let mut v = 0u64;
    for (i, &q) in reg.iter().enumerate() {
        v |= ((sim.qubit(q) >> s) & 1) << i;
    }
    v
}
fn read_bit<R: sha3::digest::XofReader>(sim: &Simulator<'_, R>, q: QubitId, s: usize) -> u64 {
    (sim.qubit(q) >> s) & 1
}

/// The scalar/dlog exceptional predicate (ADR 0016): the addend `∞` (`m==0`), the
/// accumulator `∞` (`y==0`), or the affine collision `y ≡ ±m (mod n)`.
fn scalar_exceptional(y: u64, m: u64, n: u64) -> bool {
    m == 0 || y == 0 || y == m || y == (n - m) % n
}

// ── (functional) the detector flags exactly the exceptional coordinate pairs ─

#[test]
fn detector_flags_exceptional_on_real_coords() {
    let curve = ToyCurve::new(17, 2, 2); // y²=x³+2x+2 / F_17, prime order
    let bits = curve.bits();
    let n = curve.n;

    // Craft a handful of (acc, addend) coordinate pairs with a known verdict.
    let g = curve.table[1];
    let cases: Vec<(Pt, Pt)> = vec![
        (curve.table[3], curve.table[5]), // generic: distinct x
        (curve.table[3], curve.table[3]), // doubling: P==Q (dx=0)
        (curve.table[3], ec_mul(n - 3, g, curve.a, curve.p)), // P==−Q (dx=0)
        (Pt::Inf, curve.table[5]),        // acc = ∞
        (curve.table[5], Pt::Inf),        // addend = ∞
    ];

    let (mut circ, det, _inputs) = build_detector(bits);
    let ops = circ.take_ops();
    let mut seed = sha3::Shake128::default();
    sha3::digest::Update::update(&mut seed, b"ec-exc-detector");
    let mut xof = sha3::digest::ExtendableOutput::finalize_xof(seed);
    let mut sim = Simulator::new(det.all, 1, &mut xof);
    sim.clear_for_shot();

    // Load each case on its own shot lane.
    let load = |sim: &mut Simulator<'_, _>, reg: &[QubitId], val: u64, s: usize| {
        for (i, &q) in reg.iter().enumerate() {
            if (val >> i) & 1 == 1 {
                *sim.qubit_mut(q) |= 1u64 << s;
            }
        }
    };
    for (s, &(acc, add)) in cases.iter().enumerate() {
        let (x1, y1) = coords(acc);
        let (x2, y2) = coords(add);
        load(&mut sim, &det.x1, x1, s);
        load(&mut sim, &det.y1, y1, s);
        load(&mut sim, &det.x2, x2, s);
        load(&mut sim, &det.y2, y2, s);
    }
    sim.apply_iter(ops.iter());

    for (s, &(acc, add)) in cases.iter().enumerate() {
        let dx0 = read_bit(&sim, det.dx0, s) == 1;
        let acc_inf = read_bit(&sim, det.acc_inf, s) == 1;
        let add_inf = read_bit(&sim, det.add_inf, s) == 1;
        let exceptional = dx0 || acc_inf || add_inf;
        // expected verdict from the classical points
        let (x1, _) = coords(acc);
        let (x2, _) = coords(add);
        let exp_dx0 = x1 == x2;
        let exp_acc_inf = acc == Pt::Inf;
        let exp_add_inf = add == Pt::Inf;
        assert_eq!(dx0, exp_dx0, "dx0 flag wrong (case {s})");
        assert_eq!(acc_inf, exp_acc_inf, "acc_inf flag wrong (case {s})");
        assert_eq!(add_inf, exp_add_inf, "add_inf flag wrong (case {s})");
        // case 0 is the generic (non-exceptional) pair; cases 1.. are exceptional.
        assert_eq!(exceptional, s != 0, "case {s} exceptional verdict wrong");
    }
    // The one generic pair (case 0) must be NON-exceptional.
    assert!(
        read_bit(&sim, det.dx0, 0) == 0
            && read_bit(&sim, det.acc_inf, 0) == 0
            && read_bit(&sim, det.add_inf, 0) == 0,
        "generic pair flagged exceptional"
    );
    // Ancilla cleanliness: every non-input, non-flag qubit back to |0>.
    let flags = [det.dx0, det.acc_inf, det.add_inf];
    let input_set: std::collections::HashSet<u64> = det
        .x1
        .iter()
        .chain(det.y1.iter())
        .chain(det.x2.iter())
        .chain(det.y2.iter())
        .map(|q| q.0)
        .collect();
    for q in 0..det.all as u64 {
        if input_set.contains(&q) || flags.iter().any(|f| f.0 == q) {
            continue;
        }
        assert_eq!(sim.qubit(QubitId(q)), 0, "scratch q{q} not clean");
    }
    assert_eq!(sim.phase, 0, "unexpected phase");
    eprintln!(
        "\n=== issue #28: reversible EC exceptional detector on real coordinates (ADR 0018) ==="
    );
    eprintln!(
        "  curve y²=x³+2x+2 / F_17, |⟨G⟩|=n={n} (prime); detector flags dx=0 / acc=∞ / addend=∞"
    );
    eprintln!("  crafted cases (generic, P==Q, P==−Q, acc=∞, addend=∞): all verdicts correct, ancilla clean.");
}

// ── (measurement) real-coordinate verdict == scalar model, end-to-end ───────

/// Measure the detector over ALL `(y, m)` coordinate pairs and return the
/// exceptional predicate table `E[y*n + m]` (real-coordinate verdict).
fn measure_exceptional_table(curve: &ToyCurve) -> Vec<bool> {
    let bits = curve.bits();
    let n = curve.n as usize;
    let (mut circ, det, _inputs) = build_detector(bits);
    let ops = circ.take_ops();

    let pairs: Vec<(usize, usize)> = (0..n).flat_map(|y| (0..n).map(move |m| (y, m))).collect();
    let mut e = vec![false; n * n];

    for chunk in pairs.chunks(64) {
        let mut seed = sha3::Shake128::default();
        sha3::digest::Update::update(&mut seed, b"ec-exc-measure");
        sha3::digest::Update::update(&mut seed, &(chunk[0].0 as u64).to_le_bytes());
        let mut xof = sha3::digest::ExtendableOutput::finalize_xof(seed);
        let mut sim = Simulator::new(det.all, 1, &mut xof);
        sim.clear_for_shot();
        let load = |sim: &mut Simulator<'_, _>, reg: &[QubitId], val: u64, s: usize| {
            for (i, &q) in reg.iter().enumerate() {
                if (val >> i) & 1 == 1 {
                    *sim.qubit_mut(q) |= 1u64 << s;
                }
            }
        };
        for (s, &(y, m)) in chunk.iter().enumerate() {
            let (x1, y1) = coords(curve.table[y]);
            let (x2, y2) = coords(curve.table[m]);
            load(&mut sim, &det.x1, x1, s);
            load(&mut sim, &det.y1, y1, s);
            load(&mut sim, &det.x2, x2, s);
            load(&mut sim, &det.y2, y2, s);
        }
        sim.apply_iter(ops.iter());
        for (s, &(y, m)) in chunk.iter().enumerate() {
            let exc = read_bit(&sim, det.dx0, s) == 1
                || read_bit(&sim, det.acc_inf, s) == 1
                || read_bit(&sim, det.add_inf, s) == 1;
            e[y * n + m] = exc;
        }
        // Registers/ancilla clean each shot (spot-check the addend register).
        assert_eq!(
            read_reg(&sim, &det.x2, 0),
            coords(curve.table[chunk[0].1]).0
        );
    }
    e
}

/// The ADR 0016 ladder windows: `t` windows of base `P` then `t` of base
/// `Q = [d]P`, per-window dlog base constant `c`.
fn ladder_windows(n: u64, w: u32, d: u64) -> Vec<u64> {
    let mut t = 0u32;
    while (1u64 << (w * t)) < n {
        t += 1;
    }
    let mut ws: Vec<u64> = (0..t).map(|i| fpow(2, (w * i) as u64, n)).collect();
    ws.extend((0..t).map(|j| fmul(fpow(2, (w * j) as u64, n), d, n)));
    ws
}

/// End-to-end exact failed count over the toy ladder, driven by a predicate
/// `exc(y, m)`. Mirrors `mid_ladder_bound.py::analyze` (integer survival mass with
/// a rational denominator tracked as `big^k`). Returns
/// `(failed_num, failed_den, union, add_inf_seen)`: the exact failed amplitude as a
/// reduced fraction `failed_num/failed_den`, the union bound as an `f64` (comparison
/// only), and whether any window emitted the `addend=∞` (`m==0`) entry.
fn end_to_end(
    n: u64,
    w: u32,
    windows: &[u64],
    offset: bool,
    exc: &dyn Fn(u64, u64) -> bool,
) -> (u128, u128, f64, bool) {
    let nn = n as usize;
    let big = 1u128 << w;
    let vals: Vec<u64> = if offset {
        (1..=(1u64 << w)).collect()
    } else {
        (0..(1u64 << w)).collect()
    };
    let c0 = windows[0];
    let mut clean = vec![0u128; nn];
    for &v in &vals {
        clean[fmul(v, c0, n) as usize] += 1;
    }
    let mut cden = big; // clean-mass denominator after k steps (== big^k)
                        // Track the exact failed amplitude as a reduced fraction: Σ_k fail_k/(cden_k·big).
    let mut failed_frac_num = 0u128;
    let mut failed_frac_den = 1u128;
    let mut union = 0f64;
    let mut add_inf_seen = false;

    // union uses the unrestricted distribution.
    let mut full = clean.clone();
    let mut fden = big;

    for &c in &windows[1..] {
        // exact survival step
        let mut new_clean = vec![0u128; nn];
        let mut fail_k = 0u128;
        for &v in &vals {
            let m = fmul(v, c, n);
            if m == 0 {
                add_inf_seen = true;
            }
            for (y, &mass) in clean.iter().enumerate() {
                if mass == 0 {
                    continue;
                }
                if exc(y as u64, m) {
                    fail_k += mass;
                } else {
                    new_clean[fadd(y as u64, m, n) as usize] += mass;
                }
            }
        }
        // failed += fail_k / (cden * big)
        let add_den = cden * big;
        // failed_frac += fail_k/add_den  (combine fractions)
        let g = gcd(failed_frac_den, add_den);
        let lcm = failed_frac_den / g * add_den;
        failed_frac_num = failed_frac_num * (lcm / failed_frac_den) + fail_k * (lcm / add_den);
        failed_frac_den = lcm;
        clean = new_clean;
        cden *= big;

        // union step (float is fine — comparison only)
        let mut new_full = vec![0u128; nn];
        let mut exc_k = 0u128;
        for &v in &vals {
            let m = fmul(v, c, n);
            for (y, &mass) in full.iter().enumerate() {
                if mass == 0 {
                    continue;
                }
                if exc(y as u64, m) {
                    exc_k += mass;
                }
                new_full[fadd(y as u64, m, n) as usize] += mass;
            }
        }
        union += exc_k as f64 / (fden as f64 * big as f64);
        full = new_full;
        fden *= big;
    }
    (failed_frac_num, failed_frac_den, union, add_inf_seen)
}

fn gcd(a: u128, b: u128) -> u128 {
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
}

#[test]
fn real_coord_exceptional_matches_scalar_model() {
    // Multiple real prime-order toy curves × window widths — so the confirmation
    // is not a `w=2`/single-curve artifact. (An exhaustive real-coordinate sweep
    // requires a small curve; attack-scale n≈2²⁵⁶ is covered by the scalar-model
    // union bound in `mid_ladder_bound.py`/ADR 0016, infeasible to sweep exactly.)
    // (p, a, b, d secret, [window widths]) — each with 2^w < n for the offset pin.
    let configs: &[(u64, u64, u64, u64, &[u32])] = &[
        (17, 2, 2, 7, &[2, 3, 4]),     // order 19
        (23, 1, 4, 11, &[2, 3, 4]),    // order 29
        (31, 1, 3, 13, &[2, 3, 4, 5]), // order 41
    ];
    eprintln!(
        "\n=== issue #28: real-coordinate mid-ladder residual, circuit-confirmed (ADR 0018) ==="
    );

    for &(p, a, b, d, ws) in configs {
        let curve = ToyCurve::new(p, a, b);
        let n = curve.n;
        let nn = n as usize;

        // (3) circuit-measured real-coordinate verdict == scalar/dlog predicate,
        //     for EVERY (accumulator, addend) pair of the whole group.
        let e = measure_exceptional_table(&curve);
        let mut mismatches = 0;
        for y in 0..nn {
            for m in 0..nn {
                if e[y * nn + m] != scalar_exceptional(y as u64, m as u64, n) {
                    mismatches += 1;
                }
            }
        }
        assert_eq!(
            mismatches, 0,
            "detector disagrees with the scalar/dlog model on {mismatches} pairs (n={n})"
        );
        let circ_exc = |y: u64, m: u64| e[(y as usize) * nn + (m as usize)];
        let scal_exc = |y: u64, m: u64| scalar_exceptional(y, m, n);

        eprintln!("  curve (p={p},a={a},b={b}) n={n} (prime): detector == scalar model on all {nn}×{nn} pairs (0 mismatches)");

        // (4) end-to-end mid-ladder residual over the real two-scalar ladder, at
        //     several window widths, driven by the CIRCUIT-measured predicate.
        for &w in ws {
            assert!((1u64 << w) < n, "offset pin needs 2^w < n (w={w}, n={n})");
            let windows = ladder_windows(n, w, d);
            let (std_num, std_den, std_union, std_addinf) =
                end_to_end(n, w, &windows, false, &circ_exc);
            let (off_num, off_den, off_union, off_addinf) =
                end_to_end(n, w, &windows, true, &circ_exc);
            let std_exact = std_num as f64 / std_den as f64;
            let off_exact = off_num as f64 / off_den as f64;

            assert!(
                std_exact <= std_union + 1e-12,
                "std exact exceeds union (n={n},w={w})"
            );
            assert!(
                off_exact <= off_union + 1e-12,
                "offset exact exceeds union (n={n},w={w})"
            );
            assert!(
                off_exact <= std_exact,
                "offset exact not <= standard (n={n},w={w})"
            );
            // Circuit-confirmed zero-window pin: standard hits addend=∞, offset never.
            assert!(
                std_addinf,
                "standard encoding expected a zero-window addend=∞ (n={n},w={w})"
            );
            assert!(
                !off_addinf,
                "offset must never emit addend=∞ (ADR 0015; n={n},w={w})"
            );

            // The circuit-driven end-to-end equals the pure scalar-driven one
            // (identical per-pair predicates, verified above).
            let (s_num, s_den, _, _) = end_to_end(n, w, &windows, false, &scal_exc);
            assert_eq!(
                std_num * s_den,
                s_num * std_den,
                "circuit-driven end-to-end != scalar-driven (n={n},w={w})"
            );
            eprintln!(
                "    w={w} ({} windows): exact std={std_exact:.3e}≤union{std_union:.3e}, \
                 off={off_exact:.3e}≤union{off_union:.3e}; offset addend=∞: never",
                windows.len()
            );
        }
    }
    eprintln!("  => the scalar-model exact bound (ADR 0016) holds at the CIRCUIT level over real");
    eprintln!("     coordinate arithmetic, across curves and window widths; the incomplete-affine");
    eprintln!(
        "     exceptional set is exactly the x-equality/∞ set a reversible detector measures."
    );
}
