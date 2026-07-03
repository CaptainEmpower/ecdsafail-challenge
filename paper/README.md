# paper/ — preprint working drafts

Draft materials assessing and (if pursued) writing up the scientific contribution
of this repository. **Working documents, not a finished paper.** All quantitative
claims trace to deterministic runs of the code (`just analysis`); the framing is
deliberately honest about scope (a verified *cost estimate* + rigor layer, not a
demonstrated attack, and not the qubit frontier).

| File | What it is |
|---|---|
| [`novelty-assessment.md`](novelty-assessment.md) | Honest diff vs the 2026 literature (Babbush / Chevignard / Roetteler) — what is genuinely new (machine-checked arithmetic + computed/verified completeness + reproducibility) and what is not (no new algorithm, not smallest qubit count). Decides the framing. |
| [`outline.md`](outline.md) | Section-by-section paper outline using the *methods/verified-artifact* framing, with explicit non-claims and a pre-submission TODO. |
| [`technical-report.md`](technical-report.md) | A citable standalone technical report (abstract, measured result tables, contributions, reproduction commands, honest limitations, how-to-cite). The lower-risk artifact if a full preprint isn't pursued. |

## Recommendation (see `novelty-assessment.md`)

Lead with the **rigor methodology**, not "beats the frontier". Before any
submission, do a full-text diff of Babbush et al. Appendix A (is their ZK/fuzz
already a completeness proof?) and Chevignard et al. (do they handle exceptional
cases?), settle authorship/attribution and disclosure posture, and get an
independent reproduction of the byte-identical build.
