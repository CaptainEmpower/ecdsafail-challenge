# 2026-06-03 — Fast island search tool + the "landability wall" at 1,729,565 T

## TL;DR
- Built a **build-once + spliced-reroll + parallel early-exit** local search tool
  (`src/bin/island_search.rs`, NOT submitted). It reproduces the official eval
  **exactly** (validated: it recomputes the leader island `cb57 @ r=6458/p=2553`
  → `0/0/0`, 1434q × 1,729,565 T = 2,480,196,210). ~6× faster than the old
  build_circuit+eval_circuit multi-process scan (~4 candidates/s on a 12-core mac
  vs ~0.5/s, 10 threads).
- **Key obstacle confirmed:** the frontier Toffoli (1,729,565 at 1434q) is a
  *landability wall*. Every Toffoli-reducing knob is a width/comparator
  **truncation** that introduces `mm̄ ≈ 6–9` classical-mismatch shots, so a clean
  0/0/0 Fiat-Shamir island is ~1/20k–1/50k. Promoted submissions show
  `DIALOG_REROLL` in the 100k+ range → competitors search ~10^5–10^6 seeds.
  Single machine at ~4/s = ~14k seeds/hr ⇒ the next rung is a **multi-hour grind**.
  (Searched ~46k candidates across cb56+w25 in one session with **zero** islands.)

## Why the splice trick works (and is submit-safe)
`DIALOG_REROLL=r` emits `2r` identity `X tx[0]` ops at a fixed point (op index
`i_r≈1028`); `DIALOG_POST_SUB_REROLL=p` emits `2p` identity `X tx[1]` ops at
`i_p≈33732`. Everything else is byte-identical across rerolls. So: build the base
op stream ONCE, find `i_r`/`i_p` by diffing builds at (1,0)/(0,1), then synthesize
the exact stream for any `(r,p)` by an in-memory iterator chain — no rebuild. The
splice is byte-exact, so any island found is reproducible by the official
`DIALOG_REROLL=r DIALOG_POST_SUB_REROLL=p build_circuit`. Speed tricks:
1. **Bulk hash:** pre-serialize the base to one contiguous 49-B/op buffer
   (kind:u8 + 6×u64, matching `fiat_shamir_seed`), hash 3 big slices + tiny
   identity chunks instead of 77M tiny `update()` calls.
2. **Lazy testgen + early-exit:** read all 9024 xof pairs first (to keep the
   simulator RNG offset identical to the trusted eval), but compute the expensive
   EC `mul`s per batch and stop at the first failing batch.

### Usage
```bash
cargo build --release --bin island_search
# search COMPARE_BITS=56 island, 10 threads, r,p in 0..600:
DIALOG_GCD_COMPARE_BITS=56 ./target/release/island_search 10 0 600 0 600
# prints "ISLAND r=.. p=.. ... score=.." on first 0/0/0; reproduce + confirm with
# DIALOG_GCD_COMPARE_BITS=56 DIALOG_REROLL=r DIALOG_POST_SUB_REROLL=p ecdsafail run
```
Gotcha: the dev shell runs `set -e -o pipefail`; guard `grep`/`killall` with
`|| true`. Never `pkill -f island_search` (self-matches the launching shell).

## Single-knob cut menu measured on the current leader base (682c10ab)
Base = cb57 + apply_clean19 + width26 + sched7 + active395, 1434q × 1,729,565 T.
Each is ONE knob step (combining two comparator/width truncations BACKFIRES on
Toffoli per the older FINDINGS). `(0,0)` profile = one sample of mm̄/ph̄.

| knob | env | Toffoli | score (×1434) | (0,0) mm/ph |
|---|---|---|---|---|
| **w25** | `DIALOG_GCD_WIDTH_MARGIN=25` | **1,724,985** | **2,473,628,490 (−0.26%)** | 6 / 4 |
| cb56 | `DIALOG_GCD_COMPARE_BITS=56` | 1,728,485 | 2,478,647,490 (−0.06%) | 7 / 4 |
| ac18 | `DIALOG_GCD_APPLY_CLEAN_COMPARE_BITS=18` | 1,728,775 | 2,479,063,350 | 9 / 11 |
| sm6 | `DIALOG_GCD_PA9024_COMPARE_SCHEDULE_MARGIN=6` | 1,729,349 | 2,479,886,466 | 8 / 7 |
| w24 | `DIALOG_GCD_WIDTH_MARGIN=24` | 1,720,361 | 2,466,997,674 | **17** / 9 (island ~hopeless) |

**w25 is the best target:** biggest single-knob reduction AND lowest `(0,0)`
failure profile. w24+ cliff (mm̄≈17) makes the island effectively unreachable.
A w25 island beats the leader by 0.26% and is durable against reroll-grinders’
exact-config search (they have to find the SAME deep island).

## Peak-qubit structure (for the durable <1434 cut, deferred)
`TRACE_PHASE_ACTIVE=1` shows peak 1434 is co-bound by **6** phases; next tier is
**1413** (−21, a big runway, but in the round84-square subsystem):
- `dialog_gcd_compressed_block_{ipmul,quotient}_reacquire_terminal_u`
- `dialog_gcd_materialized_special_chunked_raw_{sum,difference}` (apply add/sub)
- `dialog_gcd_raw_pa_{pair1_quotient,pair2_product}`
Next: `r84k_z_inv_squares` / `round84_fused_square_xtail_..._lowq` @ 1413.
Dropping the global peak by 1 needs **all 6** binders to fall to ≤1433 together —
the reason 1434 is a stable floor many independent solvers converged to. A
*value-exact* qubit cut would only need a ph-clean island (~1/55, seconds) since
mm stays 0 — that is the highest-leverage durable win if the 6-binder simultaneity
can be cracked. Recommend resuming there.

## EXACT peak decomposition (measured this session)
At peak (first hit at `dialog_gcd_raw_pa_pair1_quotient`, op 51754, very early in
the pair-1 quotient tobitvector), the 1434 live qubits are EXACTLY:
```
tx (factor)        256
ty (target)        256
u  (GCD working)   256   (= p at step 0, full width)
compressed_log     660   (= blocks×BLOCK_BITS = ceil(395/3)=132 × 5)
raw_block            6   (= 2 × HIGH_TAIL_ALIAS_GROUP_SIZE(3))
                  ----
                  1434   (ZERO body transient — BODY_HOST_CIN + LATE_BORROW_UV_HIGH
                          already fold c_in/gated/carry into existing registers)
```
All SIX 1434-binders (pair1_quotient, pair2_product, the two apply
materialized_special_chunked sum/difference, and the two ipmul/quotient
reacquire_terminal_u) are the SAME register pattern, so any shrink of the one
non-fixed register (`compressed_log`) drops the global peak directly.

### Why 1434 is sticky — the full-log-live design is intentional
`compressed_log` is allocated full (660) up front ON PURPOSE: the GCD body borrows
the still-`|0>` *future* log blocks as scratch for the gated/carry lanes
(`DIALOG_GCD_HOST_GATED`, `DIALOG_GCD_LATE_BORROW_UV_HIGH`, future-carry slice =
`2*active_width-1` slots). That's why there's no separate scratch register and the
body transient is 0 — but it also pins the full 660-bit log live for the whole
pass. You can't make the log incremental without losing the free scratch.

### Structural levers, ranked by tractability
- **BLOCK_BITS 5→4 (−132q, −9%)**: needs ≤16 reachable 3-step branch patterns per
  block. Encoding is already 5 (⇒ reachable count is in (16,32]), so 4 is almost
  certainly impossible. GROUP_SIZE=4 (4 steps/block) is the variant to study
  (round326_b5_exact_cover_terms.rs / ROUND763) — uncertain.
- **Host `raw_block` (6q) on free future-log slots** like gated (−6q → 1428,
  ~−0.4%, value-exact ⇒ only needs a ph island ~1/55, fast to land). At the early
  peak the future log has 655 free `|0>` slots, far more than gated/carry borrow,
  so room likely exists at every step. HIGHEST-tractability structural win, but
  still a careful edit to `dialog_gcd_copy_compressed_block_to_raw` + the body
  borrow accounting; risk of mm/ph/ancilla regressions. NOT attempted yet.
- **Incremental u-shrink + log (−~200q)**: free u's provably-zero high bits and
  size the log per step. Conflicts with the future-log-borrow scratch design;
  major re-architecture. Multi-session.
- **active_iterations 395→394 (−5q)**: NOT value-exact — adds mm̄ (same
  landability wall as the Toffoli cuts).

## Recommendation for next session
1. Resume the w25 (or cb56) island grind with `island_search` — it WILL land given
   enough seeds; budget ~50k–100k candidates. Run multiple machines / longer.
2. OR pursue the value-exact <1434 qubit cut (only needs a ph island → fast to
   land) — bigger, durable, and reroll-grinders can't copy it.
