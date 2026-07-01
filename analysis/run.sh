#!/usr/bin/env bash
# Run the full scientific-rigor suite: formal proofs + physical cost model.
set -euo pipefail
cd "$(dirname "$0")"

echo "### 1/3  Solinas modular-reduction proof (z3) ###"
python3 verify/solinas_reduction.py
echo
echo "### 2/3  Peephole / adder / comparator proofs (z3) ###"
python3 verify/peephole_identities.py
echo
echo "### 3/3  Physical fault-tolerant cost model ###"
python3 cost_model.py
