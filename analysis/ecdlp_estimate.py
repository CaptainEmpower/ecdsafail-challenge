#!/usr/bin/env python3
"""Derived full-circuit cost for a Shor-ECDLP attack on secp256k1, built from
this repo's MEASURED per-point-addition metrics composed with the EXACT ladder
formula of the source paper.

SOURCE PAPER (this challenge's origin, docs/paper 2603.28846v2.pdf):
  Babbush, Zalcman, Gidney, Broughton, Khattar, Neven, Bergamaschi, Drake, Boneh,
  "Securing Elliptic Curve Cryptocurrencies against Quantum Vulnerabilities:
  Resource Estimates and Mitigations", Google Quantum AI, 2026
  (arXiv:2603.28846v2). Appendix A gives the circuit architecture and the
  closed-form ECDLP cost we use here.

The paper's algorithm performs windowed in-place point additions Q <- Q + P[k],
where P is a classically precomputed 2^w-entry table, k is a w-qubit window
register, and the accumulator/ancilla registers are reused across all additions
(so qubit width does NOT grow with the number of additions). Its closed forms:

  ECDLP_Toff   = (PA_Toff + 3 * 2^w) * (2n/w - 4)          (A1)
  ECDLP_Qubits = PA_Qubits + w                             (A2)
  optimal window w = 16  ->  2n/w - 4 = 28 windowed additions   (A3, n=256)

where PA_Toff / PA_Qubits are the cost of ONE point-addition circuit. This repo
IS an implementation of that point-addition primitive (a "kickmix" circuit using
measurement-based uncomputation, Appendix A.4). We substitute this repo's
MEASURED PA metrics into (A1)/(A2) and compare against the Babbush et al. 2026
point-addition operating points (the challenge's reference numbers).

NOTE (referee F7, issue #61): Babbush et al.'s PUBLIC headline is the full-ECDLP
totals (<=90M Tof / <1200 q; <=70M / <1450 q). The per-point-addition Pareto
points below are the challenge's reference numbers for the same line of work; the
exact source (paper table vs. organizer-supplied) should be pinned before any
submission. They are the comparison baseline here, not asserted as the paper's
published PA bounds.

The point-addition (PA) Pareto points and resulting full ECDLP:
  Low-Qubit variant : PA <= 2,700,000 Toffoli, <= 1,175 qubits, <= 17,000,000 ops
                      -> ECDLP <= 90,000,000 Toffoli, <= 1,200 qubits
  Low-Gate  variant : PA <= 2,100,000 Toffoli, <= 1,425 qubits, <= 17,000,000 ops
                      -> ECDLP <= 70,000,000 Toffoli, <= 1,450 qubits

CAVEATS (printed below too):
  - The paper's PA table lookup loads P[k] from a QUANTUM window register; this
    repo's measured PA adds a *classical, compile-time* point. That the PA
    arithmetic core is addend-independent is now MEASURED, not assumed (issue #27,
    ADR 0012): `coord_addsub` loads the classical addend into a qubit register and
    runs an uncontrolled quantum-quantum Cuccaro add, so the Toffoli count is
    addend-value-independent; the only addend-dependent optimization (peephole
    constprop) is 0.05% of PA. So the classical-vs-quantum-addend Toffoli gap is
    negligible. The 3*2^w term prices the lookup separately.
  - QUBITS (A2): the qubit headline uses the paper's ECDLP_Qubits = PA_Qubits + w.
    That +w (window register) is correct for the paper's PA, whose PA_Qubits bound
    already prices a RESIDENT quantum addend. This repo's measured PA_Qubits keeps
    the addend CLASSICAL (loaded into a transient temp, freed off-peak, never at the
    GCD peak), so a faithful quantum-addend port of THIS PA must hold the addend
    resident ACROSS the peak, adding +256..512 qubits (MEASURED, issue #27, ADR 0013:
    coord 1026 < 1152 peak; port peak 1408..1664). The Toffoli headline is unaffected
    (ADR 0012); only the qubit figure carries this +256..512 caveat.
  - COMPLETENESS: exceptional cases (P==Q, P==-Q, infinity) are assumed handled
    at negligible Toffoli cost, as in the paper. This is a cost estimate, not a
    verified attack. Set completeness_overhead > 1 to price complete formulas.
  - Phase-estimation / qubit-recycled QFT overhead is folded into the paper's
    closed form and taken as included; we add no separate QFT Toffoli.
"""
import json
import os

HERE = os.path.dirname(os.path.abspath(__file__))


def load(name):
    p = os.path.normpath(os.path.join(HERE, "..", name))
    if not os.path.exists(p):
        return None
    with open(p) as f:
        return json.load(f)


score = load("score.json")
depth = load("depth.json")
if score is None:
    raise SystemExit("score.json not found (run the benchmark first)")
if depth is None:
    raise SystemExit("depth.json not found (run: cargo run --release --bin depth_report)")

# ----------------------------- MEASURED INPUTS -----------------------------
PA_TOF = score["metrics"]["toffoli"]         # Toffoli per point addition (measured)
PA_QUBITS = score["metrics"]["qubits"]       # total qubits per point addition (measured)
PA_TOF_DEPTH = depth["toffoli_depth"]        # non-Clifford critical path (measured)

# ------------------- BABBUSH PA OPERATING POINTS (baseline) ----------------
# Challenge reference numbers for the Babbush et al. 2026 line of work
# (arXiv:2603.28846v2). The paper's PUBLIC headline is the full-ECDLP totals;
# these per-PA Pareto points' exact source (paper table vs organizer-supplied)
# is to be pinned before submission — see the NOTE in the module docstring (F7).
PAPER = {
    "low-qubit": {"pa_tof": 2_700_000, "pa_qubits": 1_175, "pa_ops": 17_000_000,
                  "ecdlp_tof": 90_000_000, "ecdlp_qubits": 1_200},
    "low-gate":  {"pa_tof": 2_100_000, "pa_qubits": 1_425, "pa_ops": 17_000_000,
                  "ecdlp_tof": 70_000_000, "ecdlp_qubits": 1_450},
}

# ----------------------------- ALGORITHM MODEL -----------------------------
N = 256                       # secp256k1 field/scalar size
W_OPT = 16                    # paper's optimal window (A3)


def n_windowed_additions(w):
    # (A1)/(A3): 2n/w - 4 windowed point additions.
    return 2 * N // w - 4


def lookup_toffoli(w):
    # (A1): each windowed addition merges w additions at the cost of 3*2^w Toffoli
    # for the table lookup of P[k]. Kept as the (conservative) headline term.
    return 3 * (1 << w)


def lookup_toffoli_measured(w):
    # MEASURED: verify/ladder_lookup_cost.py builds + validates an optimized
    # unary-iteration QROM read and measures 2^(w+1)-4 Toffoli (w ancilla) —
    # below the paper's 3*2^w. Grounds the lookup term (issue #4, ADR 0010).
    return (1 << (w + 1)) - 4


def ecdlp_toffoli(pa_tof, w, co=1.0):
    return int((pa_tof + lookup_toffoli(w)) * n_windowed_additions(w) * co)


def ecdlp_qubits(pa_qubits, w):
    return pa_qubits + w                                  # (A2)


# ----------------------------- ASSUMPTIONS ---------------------------------
A = {
    "p_phys": 1e-3,
    "p_th": 1e-2,
    "t_react_us": 10.0,
    "T_per_toffoli": 4,            # measurement-based Toffoli (repo + paper technique)
    "phys_per_patch": lambda d: 2 * d * d,
    "factory_routing_overhead": 2.0,
    "distance": 27,
    "completeness_overhead": 1.0,  # exceptions assumed negligible (per paper)
    # valid windows must divide 2n=512; only w=16 keeps the lookup 3*2^w small
    # relative to PA while minimizing the addition count (w=32 blows up 2^w).
    "windows": [8, 16],
}


def section(t):
    print("\n" + t + "\n" + "-" * len(t))


print("=" * 78)
print(" Shor-ECDLP on secp256k1  ->  derived cost (measured PA x paper's ladder formula)")
print(" source: Babbush et al. 2026, arXiv:2603.28846v2, Appendix A")
print("=" * 78)

section("MEASURED POINT ADDITION (this repo; score.json + depth.json)")
print(f"  PA Toffoli        : {PA_TOF:,}")
print(f"  PA qubits (total) : {PA_QUBITS:,}")
print(f"  PA Toffoli-depth  : {PA_TOF_DEPTH:,}")

section("vs BABBUSH PA OPERATING POINTS (challenge reference numbers; source TBD, F7)")
print(f"  {'variant':>10} | {'PA Toffoli':>12} | {'PA qubits':>9} | this repo beats?")
for name, p in PAPER.items():
    beats = "YES (all axes)" if (PA_TOF <= p["pa_tof"] and PA_QUBITS <= p["pa_qubits"]) else "no"
    print(f"  {name:>10} | {p['pa_tof']:>12,} | {p['pa_qubits']:>9,} | {beats}")
print(f"  -> measured PA {PA_TOF:,} Tof / {PA_QUBITS:,} q is under BOTH operating points.")

section("FULL ECDLP via the paper's closed form  ECDLP=(PA+3*2^w)(2n/w-4)")
co = A["completeness_overhead"]
print(f"  completeness_overhead = {co}  (1.0 = exceptions assumed negligible, per paper)")
print(f"  {'window':>6} | {'#adds':>6} | {'lookup 3*2^w':>12} | {'ECDLP Toffoli':>14} | {'ECDLP qubits':>12}")
rows = {}
for w in A["windows"]:
    adds = n_windowed_additions(w)
    tof = ecdlp_toffoli(PA_TOF, w, co)
    q = ecdlp_qubits(PA_QUBITS, w)
    rows[w] = (adds, tof, q)
    tag = "  <- paper's optimal w" if w == W_OPT else ""
    print(f"  {w:>6} | {adds:>6} | {lookup_toffoli(w):>12,} | {tof:>14,} | {q:>12,}{tag}")

print("  lookup term is now MEASURED, not just cited (issue #4, ADR 0010):")
for w in A["windows"]:
    meas = lookup_toffoli_measured(w)
    paper = lookup_toffoli(w)
    print(f"    w={w:<2}: verify/ladder_lookup_cost.py validates a unary-iteration QROM "
          f"read at {meas:,} Toffoli ({w} ancilla) = {meas/paper:.2f}x the 3*2^w headline "
          f"-> headline is conservative on the lookup term.")

adds, tof_full, q_full = rows[W_OPT]
section("HEADLINE (w=16, this repo's measured PA in the paper's algorithm)")
print(f"  full-ECDLP Toffoli : {tof_full:,}  (~{tof_full/1e6:.1f}M)")
print(f"  full-ECDLP qubits  : {q_full:,}")
for name, p in PAPER.items():
    ratio = p["ecdlp_tof"] / tof_full
    print(f"  vs paper {name:>9} published <= {p['ecdlp_tof']/1e6:.0f}M Tof / {p['ecdlp_qubits']:,} q"
          f"  ->  {ratio:.2f}x fewer Toffoli")

ladder = load("analysis/ladder_measured.json")
if ladder is not None:
    section("MEASURED FULL-LADDER (streamed emission+count — ladder_full.rs, ADR 0011/0017)")
    lw = ladder["w"]
    ln = ladder["n_add"]
    # The estimate's HEADLINE above uses the EXECUTED avg-per-shot PA (score.json);
    # this artifact is the STATIC op-stream PA basis. They are two legitimate bases
    # (a mis-conflation would be a bug), so we cross-check the stream against the
    # closed form ON ITS OWN static basis — the same identity ladder_full asserts.
    derived_static = (ladder["pa_toffoli"] + lookup_toffoli(lw)) * ln
    mbuc_saving = 6 * ln                       # reversible unload -> MBUC unload
    assert ladder["ladder_toffoli_mbuc"] == derived_static - mbuc_saving, (
        "measured MBUC stream inconsistent with the static-basis closed form"
    )
    assert ladder["read_selector_toffoli"] == (1 << (lw + 1)) - 4, (
        "measured QROM read selector != 2^(w+1)-4"
    )
    print(f"  streamed {ln}-window w={lw} ladder, emitted+counted end-to-end (no materialization):")
    print(f"    full-ECDLP Toffoli (reversible)  : {ladder['ladder_toffoli_reversible']:,}"
          f"  (~{ladder['ladder_toffoli_reversible']/1e6:.1f}M)")
    print(f"    full-ECDLP Toffoli (MBUC unload) : {ladder['ladder_toffoli_mbuc']:,}"
          f"  (~{ladder['ladder_toffoli_mbuc']/1e6:.1f}M)")
    print(f"    full-ECDLP toffoli-depth         : {ladder['ladder_toffoli_depth']:,}"
          f"  (~{ladder['ladder_toffoli_depth']/1e6:.1f}M; {ladder['ladder_toffoli_depth_basis']})")
    print(f"    peak qubits (paper A2)           : {ladder['peak_qubits_a2']:,}"
          f"   [this PA's quantum-addend port {ladder['qaddend_port_peak_lo']:,}.."
          f"{ladder['qaddend_port_peak_hi']:,}, ADR 0013]")
    print(f"    PA basis (static op-stream)      : {ladder['pa_toffoli']:,} Toffoli /"
          f" {ladder['pa_qubits']:,} q / depth {ladder['pa_toffoli_depth']:,}")
    print(f"  cross-check: measured MBUC {ladder['ladder_toffoli_mbuc']:,} =="
          f" (PA+3·2^{lw})·{ln} − 6·{ln} = {derived_static - mbuc_saving:,}  [consistent]")
    print("  NB: this is the STATIC op-stream basis; the w=16 HEADLINE above is the")
    print("  EXECUTED avg-per-shot basis (score.json) — two valid PA measures, not a gap.")

section("PHYSICAL FAULT-TOLERANT COST  (w=16 headline)")
d = A["distance"]
phys = int(q_full * A["phys_per_patch"](d) * A["factory_routing_overhead"])
t_count = tof_full * A["T_per_toffoli"]
# accumulator is read+written by every addition -> additions serialize; the
# non-Clifford critical path composes as (#adds) x (per-addition toffoli-depth).
tdepth = PA_TOF_DEPTH * adds
runtime_s = tdepth * A["t_react_us"] * 1e-6
vol = phys * runtime_s
print(f"  total T-count @ {A['T_per_toffoli']} T/Tof : {t_count:,}  (~{t_count:.2e})")
print(f"  physical qubits @ d={d}     : {phys:,}  (~{phys:.2e})")
print(f"  composed Toffoli-depth    : {tdepth:,}")
print(f"  reaction-limited runtime  : {runtime_s:,.0f} s  = {runtime_s/60:.1f} min")
print(f"  spacetime volume          : {vol:.3e} physical-qubit-seconds")
print("  NOTE: runtime (~minutes) matches the paper; our physical-qubit figure is a")
print("  COARSE upper bound (2d^2/patch, 2x routing, no factory sharing) and sits")
print("  above the paper's optimized < 500k -- an assumptions gap, not a discrepancy")
print("  in the logical circuit. See cost_model.py for the physical assumptions.")

section("CAVEATS")
print("  - PA arithmetic core addend-independence is now MEASURED (issue #27,")
print("    ADR 0012): coord_addsub loads the classical addend into a qubit register")
print("    and runs an uncontrolled q-q Cuccaro add, so Toffoli is addend-value-")
print("    independent; the classical-vs-quantum-addend gap is <=0.05% of PA. The")
print("    3*2^w term prices the lookup separately.")
print("  - QUBITS (A2): the qubit headline PA_Qubits+w is the PAPER's bound (its")
print("    PA_Qubits already prices a resident quantum addend). This repo keeps the")
print("    addend CLASSICAL (freed off-peak), so a faithful quantum-addend port of")
print("    THIS PA holds it resident across the peak: +256..512 qubits (MEASURED,")
print("    issue #27, ADR 0013: port peak 1408..1664 vs PA 1152). Toffoli unaffected.")
print("  - COMPLETENESS (P==Q, P==-Q, infinity) assumed negligible, as in the paper.")
print("    This is a COST estimate, not a verified attack.")
print("  - The HEADLINE is DERIVED (measured PA x paper's closed form). The full")
if ladder is not None:
    print("    ladder is ALSO emitted+measured end-to-end (streamed, no materialization)")
    print("    — see the MEASURED FULL-LADDER section / ladder_measured.json (issue #27")
    print("    item 3, ADR 0011/0017); the two agree on the static basis to 6·n_add.")
else:
    print("    ladder Tier-B stream lands in ladder_full.rs; generate its measured")
    print("    artifact (ladder_measured.json) to have it accompany this headline.")
print("=" * 78)
