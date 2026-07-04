#!/usr/bin/env bash
# Kani (bit-precise BMC) proofs that bind to the REAL Rust types/functions.
# Harnesses live in src/kani_proofs.rs and src/point_add/mbuc_kani.rs, gated
# behind #[cfg(kani)] so the normal build and benchmark.sh never compile them.
# Requires: cargo kani (v0.66+).
set -euo pipefail
cd "$(git rev-parse --show-toplevel 2>/dev/null || echo "$(dirname "$0")/../..")"

# solinas_add_*      — the arithmetic contract on real integer types (a hand-written
#                      twin of mod_add_qq's control flow; abstract-adder model).
# mbuc_fast_adder_*  — bound to the REAL emitter + REAL simulator (ADR 0030): emits
#                      cuccaro_add_fast via the B builder and runs src/sim.rs, proving
#                      functional + a-preserved + ancilla-clean + phase-clean over all
#                      inputs and all measurement outcomes at small width. Closes the
#                      copy↔emitter gap (referee F2) on the Rust side. The exhaustive
#                      concrete twin runs in `cargo test` (mbuc_kani::shadow).
for h in solinas_add_u64 solinas_add_u256 mbuc_fast_adder_width2 mbuc_fast_adder_width3; do
  echo "### cargo kani --harness $h ###"
  cargo kani --harness "$h" 2>&1 | grep -E "VERIFICATION|failed|Verification Time" || true
  echo
done
