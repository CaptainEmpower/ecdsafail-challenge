#!/usr/bin/env bash
# Run the full scientific-rigor suite: formal proofs + physical cost model.
set -euo pipefail
cd "$(dirname "$0")"

echo "### 1/11  Solinas modular-reduction proof (z3) ###"
python3 verify/solinas_reduction.py
echo
echo "### 2/11  Peephole / adder / comparator proofs (z3) ###"
python3 verify/peephole_identities.py
echo
echo "### 3/11  Reference kickmix adder validation (source-paper artifacts) ###"
python3 verify/validate_reference_adders.py
echo
echo "### 4/11  Constructed controlled table-lookup validation (ladder QROM primitive) ###"
python3 verify/controlled_lookup.py
echo
echo "### 5/11  Windowed-lookup (QROM) cost: measured unary-iteration read (issue #4) ###"
python3 verify/ladder_lookup_cost.py
echo
echo "### 6/11  Empirical adder-completeness collision rate (issue #5, Path A) ###"
python3 verify/completeness_collision_rate.py
echo
echo "### 7/11  Direct-lookup first window: circuit-level ∞-start removal (issue #5a) ###"
python3 verify/direct_lookup_init.py
echo
echo "### 8/11  Offset window encoding: remove the zero-window ∞ term (issue #5b) ###"
python3 verify/offset_window_encoding.py
echo
echo "### 9/11  Exact end-to-end mid-ladder exceptional bound (issue #28) ###"
python3 verify/mid_ladder_bound.py
echo
echo "### 10/11  Physical fault-tolerant cost model ###"
python3 cost_model.py
echo
echo "### 11/11  Derived full-ECDLP cost (measured primitive x paper's ladder) ###"
python3 ecdlp_estimate.py
