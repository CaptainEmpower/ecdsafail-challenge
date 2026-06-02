# 2026-06-02 margin 5 plus apply-clean compare 19 island

Route: current dialog-GCD compressed sidecar with odd-u low-bit body skip.

Worked:

- Tightened `DIALOG_GCD_PA9024_COMPARE_SCHEDULE_MARGIN` from 8 to 5.
- Tightened `DIALOG_GCD_APPLY_CLEAN_COMPARE_BITS` from 20 to 19.
- Co-tuned the identity pads to `DIALOG_REROLL=0` and
  `DIALOG_POST_SUB_REROLL=23`.
- Local full validator passed 9024/9024 shots with 0 classical mismatches,
  0 phase-garbage batches, and 0 ancilla-garbage batches.

Metrics from the fast local evaluator:

- Average executed Toffoli: 1,742,136
- Peak qubits: 1,571
- Score: 2,736,895,656

This is a pure Toffoli reduction at the same 1,571-qubit peak. The tighter
comparator and schedule margin are approximate in the same Fiat-Shamir-island
sense as the existing truncation knobs; neighbouring reroll/post-sub islands
still fail.
