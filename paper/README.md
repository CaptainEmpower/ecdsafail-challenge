# paper/ — preprint working drafts

Draft materials assessing and (if pursued) writing up the scientific contribution
of this repository. **Working documents, not a finished paper.** All quantitative
claims trace to deterministic runs of the code (`just analysis`); the framing is
deliberately honest about scope (a verified *cost estimate* + rigor layer, not a
demonstrated attack, and not the qubit frontier).

| File | What it is |
|---|---|
| [`novelty-assessment.md`](novelty-assessment.md) | Honest **full-text** diff vs the 2026 literature (Babbush arXiv:2603.28846 / Han Luo arXiv:2604.02311 / Chevignard ePrint 2026/280 / Roetteler) — what is genuinely new (machine-checked arithmetic + computed/verified completeness + reproducibility) and what is not (no new algorithm, not smallest qubit count). Decides the framing. |
| [`outline.md`](outline.md) | Section-by-section paper outline using the *methods/verified-artifact* framing, with explicit non-claims and a pre-submission TODO. |
| [`technical-report.md`](technical-report.md) | A citable standalone technical report (abstract, measured result tables, contributions, reproduction commands, honest limitations, how-to-cite). The lower-risk artifact if a full preprint isn't pursued. |

## Recommendation (see `novelty-assessment.md`)

Lead with the **rigor methodology**, not "beats the frontier". The full-text diff
is **done**: Babbush App. A is a zero-knowledge proof *of resource costs* over a
≥99% Fiat–Shamir fuzz sample — not a completeness proof, and the paper never treats
the point at infinity; neither space-optimized paper (Han Luo 2604.02311;
Chevignard 2026/280) machine-checks arithmetic or handles the exceptional cases.
The machine-checked-correctness + verified-completeness delta holds and sharpens.
Remaining before submission: settle authorship/attribution and disclosure posture,
and get an independent reproduction of the byte-identical build.
