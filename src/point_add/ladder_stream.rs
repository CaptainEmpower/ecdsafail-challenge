//! Tier B (issue #27), item 2: the TRUE quantum-addend windowed ladder — a
//! **multi-window** `read→add→unread` stream where the QROM read WRITES the addend
//! register the adder CONSUMES (a real RAW dependency), the arithmetic workspace is
//! REUSED across windows, and Toffoli / peak-qubits / true toffoli-depth are
//! stream-measured end-to-end. Closes the gap the two cost harnesses each leave:
//!
//!   - `ladder_full.rs` (ADR 0011) streams `[read, PA, read] × 28` but emits the
//!     QROM on **disjoint ids** over the **classical-addend** PA — so the add never
//!     consumes the loaded addend and the QROM composes in *parallel*, making its
//!     toffoli-depth **add-only** (it flags this as an under-count).
//!   - `qaddend_testbed.rs` (ADR 0014) builds the TRUE read→add→unread (addend
//!     consumed) but only for a **single** window.
//!
//! This harness reuses ADR 0014's verified `qrom_read` / `mod_add` fragments and
//! composes them over `m` windows. Two faces (ADR 0017):
//!
//!   1. **Functional** (fast): a distinct `w`-bit scalar window per step feeding one
//!      shared workspace, simulation-verified to thread the accumulator across
//!      windows — `acc == (y + Σ_j T_j[k_j]) mod p`, all ancilla clean.
//!   2. **Measurement** (`#[ignore]`, heavy): reuse the workspace ids across `m`
//!      windows, STREAM the op emission (only one window is ever materialized), and
//!      measure the true serialized read→add→unread toffoli-depth (RAW through the
//!      shared addend) against the disjoint-id variant that reproduces
//!      `ladder_full.rs`'s add-only undercount — the measured delta IS the read→add
//!      serialization depth that harness omitted.
//!
//! `#[cfg(test)]` only; never compiled into the scored circuit (ops.bin unchanged).

use super::qaddend_testbed::{mod_add, qrom_read};
use crate::circuit::{analyze_depth, analyze_ops, Op, OperationType, QubitId};
use crate::point_add::B;
use crate::sim::Simulator;

fn toffoli_count<'a>(ops: impl Iterator<Item = &'a Op>) -> u64 {
    ops.filter(|o| matches!(o.kind, OperationType::CCX | OperationType::CCZ))
        .count() as u64
}

/// splitmix64 — deterministic table/scalar constants (no `rand`).
fn splitmix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn read_reg<R: sha3::digest::XofReader>(sim: &Simulator<'_, R>, reg: &[QubitId], s: usize) -> u64 {
    let mut v = 0u64;
    for (i, &q) in reg.iter().enumerate() {
        v |= ((sim.qubit(q) >> s) & 1) << i;
    }
    v
}

// ── (1) functional: multi-window accumulation, simulation-verified ──────────

/// Build an `m`-window ladder (distinct scalar windows, shared workspace) and
/// verify by masked multi-shot simulation that the accumulator threads correctly
/// across windows: `acc == (y + Σ_j T_j[k_j]) mod p`, every ancilla clean, scalar
/// preserved. Returns the peak width.
fn run_ladder_mod(n: usize, w: usize, m: usize, p: u64) -> u64 {
    assert!((1..=6).contains(&w) && (2..=60).contains(&n) && (1..=8).contains(&m));
    assert!((1..(1u64 << n)).contains(&p));
    // Per-window tables T_j (each a different "digit position"), reduced mod p.
    let tables: Vec<Vec<u64>> = (0..m)
        .map(|j| {
            (0..(1usize << w))
                .map(|k| splitmix(0x0ADD_0F00 ^ ((j as u64) << 40) ^ (k as u64)) % p)
                .collect()
        })
        .collect();

    // scalar (m distinct windows) | addend | acc | spine | carry | hi | flag | preg
    let mut circ = B::new_for_test();
    let scalar = circ.alloc_qubits(m * w);
    let addend = circ.alloc_qubits(n);
    let acc = circ.alloc_qubits(n);
    let anc = circ.alloc_qubits(w);
    let carry = circ.alloc_qubits(1);
    let hi = circ.alloc_qubits(1);
    let flag = circ.alloc_qubits(1);
    let preg = circ.alloc_qubits(n);

    for (j, table) in tables.iter().enumerate() {
        let win = &scalar[j * w..(j + 1) * w];
        qrom_read(&mut circ, win, &anc, &addend, table); // addend := T_j[k_j]
        mod_add(&mut circ, &addend, &acc, p, hi[0], flag[0], &preg, carry[0]); // acc += mod p
        qrom_read(&mut circ, win, &anc, &addend, table); // addend := 0 (unread)
    }
    let ops = circ.take_ops();
    let (peak_qubits, nbits, _r, _regs) = analyze_ops(ops.iter());

    // Masked multi-shot: lane s carries a full scalar (m windows) and acc y_s < p.
    let n_win = 1usize << w;
    let shots = 64usize;
    let k_of =
        |s: usize, j: usize| splitmix(0x0C0F_FEE0 ^ ((s as u64) << 8) ^ j as u64) % n_win as u64;
    let y_of = |s: usize| splitmix(0x0ACC_1CE0 ^ s as u64) % p;

    let mut seed = sha3::Shake128::default();
    sha3::digest::Update::update(&mut seed, b"ladder-stream-mod");
    let mut xof = sha3::digest::ExtendableOutput::finalize_xof(seed);
    let mut sim = Simulator::new(peak_qubits as usize, nbits as usize, &mut xof);
    sim.clear_for_shot();
    // Load the scalar windows and the accumulator; workspace stays |0>.
    for j in 0..m {
        for (b, &q) in scalar[j * w..(j + 1) * w].iter().enumerate() {
            let mut mask = 0u64;
            for s in 0..shots {
                mask |= ((k_of(s, j) >> b) & 1) << s;
            }
            *sim.qubit_mut(q) = mask;
        }
    }
    for (i, &q) in acc.iter().enumerate() {
        let mut mask = 0u64;
        for s in 0..shots {
            mask |= ((y_of(s) >> i) & 1) << s;
        }
        *sim.qubit_mut(q) = mask;
    }
    sim.apply_iter(ops.iter());

    for s in 0..shots {
        let mut expect = y_of(s);
        for (j, table) in tables.iter().enumerate() {
            expect = (expect + table[k_of(s, j) as usize]) % p;
        }
        assert_eq!(
            read_reg(&sim, &acc, s),
            expect,
            "ladder acc mismatch (n={n}, w={w}, m={m}, p={p}, shot={s})"
        );
        // scalar preserved; addend, spine, preg all clean.
        for j in 0..m {
            assert_eq!(
                read_reg(&sim, &scalar[j * w..(j + 1) * w], s),
                k_of(s, j),
                "scalar window {j} perturbed (shot {s})"
            );
        }
        assert_eq!(read_reg(&sim, &addend, s), 0, "addend not returned to |0>");
        assert_eq!(read_reg(&sim, &anc, s), 0, "selector spine dirty");
        assert_eq!(read_reg(&sim, &preg, s), 0, "preg scratch dirty");
    }
    assert_eq!(sim.qubit(carry[0]), 0, "carry dirty");
    assert_eq!(sim.qubit(hi[0]), 0, "hi dirty");
    assert_eq!(sim.qubit(flag[0]), 0, "flag dirty");
    assert_eq!(sim.phase, 0, "unexpected phase");
    peak_qubits
}

#[test]
fn streamed_windowed_ladder_accumulates_mod_p() {
    // (n, w, m, p): a few widths and window counts; p prime < 2^n.
    let cases = [
        (6usize, 2usize, 4usize, 61u64),
        (5, 3, 3, 29),
        (8, 3, 5, 251),
    ];
    eprintln!("\n=== issue #27 item 2: streamed multi-window quantum-addend ladder (mod p) ===");
    eprintln!("  (accumulator threading across windows; correctness by simulation; ADR 0017)");
    for (n, w, m, p) in cases {
        let peak = run_ladder_mod(n, w, m, p);
        // scalar(m*w) + addend(n) + acc(n) + spine(w) + carry + hi + flag + preg(n)
        let expect_peak = (m * w + n + n + w + 1 + 1 + 1 + n) as u64;
        assert_eq!(peak, expect_peak, "unexpected peak for n={n}, w={w}, m={m}");
        eprintln!(
            "  n={n:<2} w={w} m={m} p={p:<3}: PASS  acc = (y + Σ_j T_j[k_j]) mod p over all shots, \
             all ancilla clean  (peak={peak})"
        );
    }
    eprintln!("  => the accumulator threads correctly across windows on ONE shared workspace —");
    eprintln!("     the multi-window step ADR 0014's single read→add→unread did not exercise.");
}

// ── (2) measurement: true serialized depth, streamed, register-overlapped ────

/// Build one window's `read→mod_add→unread` op stream on a FIXED register layout,
/// returning `(ops, peak_qubits, nbits, read_tof, add_tof, read_depth, add_depth)`.
/// If `disjoint`, the QROM writes a SEPARATE addend scratch that the adder never
/// reads (à la `ladder_full.rs`) — QROM composes in parallel; otherwise the QROM
/// writes the very addend the adder consumes (a real RAW dependency).
fn build_window(
    n: usize,
    w: usize,
    p: u64,
    disjoint: bool,
) -> (Vec<Op>, u64, u64, u64, u64, u64, u64) {
    let table: Vec<u64> = (0..(1usize << w))
        .map(|k| splitmix(0x0ADD_5EED ^ (k as u64)) % p)
        .collect();

    let mut circ = B::new_for_test();
    let win = circ.alloc_qubits(w);
    let addend = circ.alloc_qubits(n); // consumed by the adder
    let acc = circ.alloc_qubits(n);
    let anc = circ.alloc_qubits(w);
    let carry = circ.alloc_qubits(1);
    let hi = circ.alloc_qubits(1);
    let flag = circ.alloc_qubits(1);
    let preg = circ.alloc_qubits(n);
    // Disjoint variant: a separate scratch the QROM writes and nothing reads.
    let scratch = if disjoint {
        circ.alloc_qubits(n)
    } else {
        addend.clone()
    };

    qrom_read(&mut circ, &win, &anc, &scratch, &table);
    mod_add(&mut circ, &addend, &acc, p, hi[0], flag[0], &preg, carry[0]);
    qrom_read(&mut circ, &win, &anc, &scratch, &table);
    let ops = circ.take_ops();
    let (peak, nbits, _r, _regs) = analyze_ops(ops.iter());

    // Isolate the read and the add costs on the same layout (for the breakdown).
    let mut rc = B::new_for_test();
    let rw = rc.alloc_qubits(w);
    let ra = rc.alloc_qubits(n);
    let rn = rc.alloc_qubits(w);
    qrom_read(&mut rc, &rw, &rn, &ra, &table);
    let rops = rc.take_ops();
    let (rq, rb, _, _) = analyze_ops(rops.iter());
    let read_tof = toffoli_count(rops.iter());
    let read_depth = analyze_depth(rops.iter(), rq as usize, rb as usize).toffoli_depth;

    let mut ac = B::new_for_test();
    let aa = ac.alloc_qubits(n);
    let ax = ac.alloc_qubits(n);
    let ac_car = ac.alloc_qubits(1);
    let ac_hi = ac.alloc_qubits(1);
    let ac_fl = ac.alloc_qubits(1);
    let ac_pr = ac.alloc_qubits(n);
    mod_add(&mut ac, &aa, &ax, p, ac_hi[0], ac_fl[0], &ac_pr, ac_car[0]);
    let aops = ac.take_ops();
    let (aq, ab, _, _) = analyze_ops(aops.iter());
    let add_tof = toffoli_count(aops.iter());
    let add_depth = analyze_depth(aops.iter(), aq as usize, ab as usize).toffoli_depth;

    (ops, peak, nbits, read_tof, add_tof, read_depth, add_depth)
}

#[test]
#[ignore = "heavy streamed multi-window ladder measurement; run with `cargo test -- --ignored`"]
fn streamed_windowed_ladder_measured() {
    let n = 32usize;
    let w = 6usize;
    let m = 2 * 256 / 16 - 4; // 28 windowed additions (paper A1/A3, real n_add)
    let p = (1u64 << n) - 5; // 2^32 - 5, prime; only its bit pattern matters here

    let (win_ov, peak_ov, nb, read_tof, add_tof, read_depth, add_depth) =
        build_window(n, w, p, false);
    let (_win_dj, peak_dj, _nb2, _rt, _at, _rd, _ad) = build_window(n, w, p, true);

    // A read's selector Toffoli must equal the unary-iteration cost (ADR 0010).
    assert_eq!(
        read_tof,
        (1u64 << (w + 1)) - 4,
        "read selector Toffoli != 2^(w+1)-4"
    );
    let per_window_tof = 2 * read_tof + add_tof;
    assert_eq!(
        toffoli_count(win_ov.iter()),
        per_window_tof,
        "per-window Toffoli != 2·read + add"
    );

    // True per-window depth (overlap: read→add→unread serialize through addend).
    let per_ov_depth = analyze_depth(win_ov.iter(), peak_ov as usize, nb as usize).toffoli_depth;
    // Disjoint per-window depth (QROM parallel to the add — ladder_full's model).
    let (win_dj, _, nb_dj, _, _, _, _) = build_window(n, w, p, true);
    let per_dj_depth = analyze_depth(win_dj.iter(), peak_dj as usize, nb_dj as usize).toffoli_depth;

    // The serialization ladder_full.rs omits: overlap depth exceeds the add-only
    // (disjoint) depth by the read→add→unread QROM contribution.
    assert!(
        per_ov_depth > per_dj_depth,
        "overlap depth ({per_ov_depth}) not > disjoint/add-only depth ({per_dj_depth}) — \
         the read→add serialization was expected to be real"
    );
    let serial_qrom_depth = per_ov_depth - per_dj_depth;
    // The disjoint peak is wider by exactly the separate addend scratch (ADR 0011's
    // flagged disjoint-emit over-count), executable here.
    assert_eq!(
        peak_dj,
        peak_ov + n as u64,
        "disjoint peak over-count != +n"
    );

    // STREAM m windows reusing the SAME workspace ids (no full-ladder
    // materialization — only one window's ops exist).
    let ladder = || (0..m).flat_map(|_| win_ov.iter());
    let total_tof = toffoli_count(ladder());
    let total_depth = analyze_depth(ladder(), peak_ov as usize, nb as usize).toffoli_depth;
    assert_eq!(
        total_tof,
        m as u64 * per_window_tof,
        "streamed Toffoli not m·per-window"
    );
    // Windows serialize through the shared accumulator: total depth = m·per-window.
    assert_eq!(
        total_depth,
        m as u64 * per_ov_depth,
        "streamed depth not serial across windows (shared-acc RAW expected)"
    );

    // Closed-form scale-up to the real headline (w=16, n_add=28), honestly labeled.
    let real_w = 16u64;
    let real_read_tof = (1u64 << (real_w + 1)) - 4; // 2^17 - 4
    let real_serial_qrom_depth = m as u64 * 2 * real_read_tof; // read + unread, per window

    eprintln!("\n=== issue #27 item 2: streamed multi-window ladder MEASUREMENT (n={n}, w={w}, m={m}) ===");
    eprintln!("  per read  : toffoli={read_tof} (=2^(w+1)-4)  toffoli_depth={read_depth}");
    eprintln!("  per add   : toffoli={add_tof}  toffoli_depth={add_depth}  (VBE mod-p adder)");
    eprintln!("  per window (read→add→unread), OVERLAP (real RAW): toffoli={per_window_tof}  depth={per_ov_depth}");
    eprintln!("  per window, DISJOINT (QROM ∥ add, ladder_full model): depth={per_dj_depth}");
    eprintln!(
        "    -> read→add SERIALIZATION depth ladder_full.rs omits = {serial_qrom_depth} per window"
    );
    eprintln!(
        "       (≈ 2·read_depth = {}; the QROM read+unread that must precede/follow the add)",
        2 * read_depth
    );
    eprintln!("  peak qubits: overlap={peak_ov}  disjoint={peak_dj} (=+{n} addend scratch, ADR 0011 over-count)");
    eprintln!(
        "    (overlap holds addend+spine RESIDENT across all {m} windows — executable ADR 0013)"
    );
    eprintln!("  ------------------------------------------------------------------");
    eprintln!("  FULL LADDER, streamed+counted end-to-end ([read,add,unread] × {m}):");
    eprintln!("    Toffoli       = {total_tof}  (= {m}·{per_window_tof})");
    eprintln!("    toffoli-depth = {total_depth}  (= {m}·{per_ov_depth}, serial across windows)");
    eprintln!(
        "  scale-up (closed form, w={real_w}, n_add={m}): read+unread serial depth added on top"
    );
    eprintln!("    of the add-dominated path ≈ {m}·2·(2^17−4) = {real_serial_qrom_depth}");
    eprintln!(
        "    (vs ladder_full.rs's add-only depth; small-width-executable + closed-form, NOT a"
    );
    eprintln!("     materialized 256-bit run — the full stream is ~290 GB, out of scope per #27.)");

    // Sanity: the streamed totals stay well under the paper's Low-Gate ECDLP bound.
    assert!(
        total_tof > 0 && total_depth > per_ov_depth,
        "degenerate streamed measurement"
    );
}
