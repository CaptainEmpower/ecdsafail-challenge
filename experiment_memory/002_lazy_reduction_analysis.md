# Experiment 002 — Lazy Reduction Analysis (2026-04-15, tick 2)

No commit this tick. Analysis only — documented so next tick can execute.

## The idea
In `mod_mul_add_qq`, each of 256 iterations calls `cmod_add_qq` → `mod_add_qq` which costs 8n CCX (add a, add c, csub c, cmp_lt_into). The reduction (add c + csub c + cmp_lt_into) is 6n of those 8n. If we skip reductions and batch them, we save.

## Cost model (verified)
- Current `mod_mul_add_qq` per mul ≈ `256·10n + 2(n-1)·4n` = 18n² − 8n ≈ 1.18M Toffoli.
- Lazy k=K, batch=B: every B iters, do a reducer (cost R). Per iter: load_f (n) + (n+K)-bit cuccaro (2n+2K) + unload_f (n) = 4n+2K. Plus reducer cost R amortized over B iters = R/B.
- Per-iter new: (4n+2K) + R/B. Current: 10n.
- Savings per iter: 6n - 2K - R/B.

## The blocker: flag uncompute
The reducer for `acc_wide ∈ [0, 2^K·p) → [0, p)` needs a reduction flag, which has a **2-unknowns-1-equation uncompute barrier** (see memory/dead_end_mod_add_bit0.md). A conventional `cmp_lt_into` uncompute costs 2(n+K), which eats most of the savings.

## Possible flag uncompute strategies
1. **cmp_lt_into with a witness** — use the current `a` register (the load ancilla `f`) as the cmp witness. The mod_add_qq identity `flag = (acc_final < a_orig)` works IF a is the actual addend and < p. For a reducer (a = 0), identity fails. **Cannot reuse mod_add_qq directly.**

2. **Defer flag uncompute to end of mul** — allocate a flag per reducer call, leave them live, uncompute at the end via a single batch cmp. Problem: the batch flag is a sum-of-flags, but uncomputing that requires knowing `original acc_wide - final acc`, and original is gone.

3. **Use an IN-BAND flag** — co-opt acc_wide[n+K-1] (the top bit) as the flag directly, never making a separate ancilla. The cadd/csub must not write to this bit during the op. Doable with a custom primitive that skips position n+K-1 during the cuccaro. Saves the 2n flag uncompute entirely.

4. **K=9 single final reducer** — batch ALL 256 adds, reduce once at end. Final reducer can use a binary sub chain (log2(257)=9 steps), each a cond sub of 2^j·p. Flag uncompute per step is still 2n CCX via cmp, but only 9 steps total. Total reducer cost: ~54n ≈ 14k Toffoli. **This is the highest-EV path.**

## Next-tick plan for K=9 lazy reduction
1. Alloc `acc_pad = [0u9]` (9 fresh ancillas) inside `mod_mul_add_qq`. `acc_wide = acc ++ acc_pad`.
2. Replace `cmod_add_qq` inner loop with unreduced version:
   - Load `f = y[i] & tmp` into n-bit register.
   - Extend f to (n+9) bits (top 9 bits = 0 ancillas).
   - `add_nbit_qq(f_ext, acc_wide)` — (n+9)-bit cuccaro.
   - Unload.
3. At end of loop (before halving tmp back): reduce `acc_wide` using 9 binary steps:
   - For j in (0..9).rev(): if `acc_wide >= 2^j · p` then `acc_wide -= 2^j · p`.
   - Constants `2^j · p` exceed U256 for j ≥ 1. Workaround: store each as `(low: U256, high_bits_pattern)` where high_bits_pattern is a count that indicates shift. Or: store `2^j · p` as a slice of bit indices.
   - Alternative: use an iterated approach — sub p, sub p, sub p, ... up to 256 times, with early exit via push_condition on... nothing (conditions are QubitIds). Won't work.
4. After reduction, `acc_pad` should all be 0. Free via `assert_zero_and_free_vec`.
5. Verify `mod_sub_qq` via `emit_inverse` still works — emit_inverse reverses the modified flow.

## Constant-representation problem
The binary reducer needs `2^j · p` for j ∈ [1, 9], which don't fit in U256. Options:
- Represent as `{bit_positions: Vec<usize>}` and write a custom `cadd_by_bit_pattern` that takes a list of set-bit positions. Cuccaro_add then runs on a register loaded at those positions.
- Shift the register instead: to sub `2^j · p`, sub `p` from `acc_wide[j..]` (subslice). This is what we want! `acc_wide[j..n+j]` is a 256-bit slice; sub `p` from it using existing `sub_nbit_const`. Cost: standard 2n CCX per step. 9 steps × (2n cmp + 2n csub + 2n flag-uncompute) = 54n total. **This is the clean path.**

## Risk
- Complexity: ~100 lines of new code.
- Correctness of shifted sub (slice subtraction interactions with carries).
- `emit_inverse` must handle the new structure cleanly (the new mod_mul_add_qq must be reversible at gate level).

## Conservative expected savings
- Per-iter save: 6n - 2·9 = 1518. ×256 iters = 388k.
- Reducer cost: 54n ≈ 13.8k.
- Net per mul: ~374k. × 5 muls = **1.87M Toffoli saved ≈ 10.1% reduction.**
- New best estimate: 18.5M → 16.6M Toffoli.
- Additional qubits: +9 (well within budget 3083 → 3092, cap 3237).
