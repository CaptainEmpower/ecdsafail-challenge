# ADR 0023 — External referee review and remediation index (issue #62)

**Status:** Accepted — records an independent skeptical review of the codebase
and its Markdown statements, and the remediation plan it drives. The review
artifact is [`paper/REVIEW.md`](../../paper/REVIEW.md).
**Date:** 2026-07-03

## Context

The repository's written claims (the `paper/` drafts, `analysis/scientific-value.md`,
`analysis/completeness_argument.md`, and the ADR trail) had accumulated a layered
rigor narrative — machine-checked arithmetic, computed/verified completeness, a
demonstrated toy-scale attack — largely reviewed only by the authors and the
qodo bot. An independent referee pass was needed to check whether the top-line
framing matches what the code establishes, *before* any preprint submission (a
pre-submission item already named in `novelty-assessment.md`).

## Decision

Accept the external review and treat its findings as the pre-submission
remediation backlog. The review (`paper/REVIEW.md`) rebuilt and re-scored the
circuit, re-ran the full analysis suite, and verified the external citations.

**Confirmed (no action):** the headline score (**1,364,230 Toffoli × 1,152 q**,
9024/9024 shots pass) reproduces independently; the z3 integer-level Solinas
proof is sound against the real secp256k1 prime; the numeric pipeline is
internally consistent; the three external citations are real. The ECDLP's
hardness is untouched.

**Findings (F1–F8)** — all concern top-line framing outrunning the
already-correct buried caveats, plus two factual citation errors. They are
grouped into three focused PRs, each with its own ADR:

| Findings | Theme | PR / ADR | Issues |
|---|---|---|---|
| F3 | Extend z3 adder/comparator proofs from ≤64-bit to production 256/257-bit (a real, fast fix) | ADR 0024 | [#58](https://github.com/CaptainEmpower/ecdsafail-challenge/issues/58) |
| F6, F7, F8 | Citation & metric-wording accuracy (Chevignard P-224/P-256; published-vs-private PA bounds; "peak qubits") | ADR 0025 | [#61](https://github.com/CaptainEmpower/ecdsafail-challenge/issues/61) |
| F1, F2, F4, F5 | Honest-scope framing (plain-vs-fast adder; Kani copy; toy-scale "demonstrated attack"; "exact" bound) | ADR 0026 | [#57](https://github.com/CaptainEmpower/ecdsafail-challenge/issues/57), [#59](https://github.com/CaptainEmpower/ecdsafail-challenge/issues/59), [#60](https://github.com/CaptainEmpower/ecdsafail-challenge/issues/60) |

See `paper/REVIEW.md` for the full report with file:line evidence.

## Consequences

- **The framing debt is made explicit and tracked,** rather than discovered by a
  reviewer at submission time. F3 is closed by proof; F1/F2/F4/F5/F6/F7/F8 are
  closed by making the prose match the artifact.
- **Consistent with [ADR 0001](0001-analysis-layer-isolated-from-score.md):** the
  review and its remediations touch only the analysis/documentation layer and the
  z3 suite; the scored secp256k1 circuit is byte-identical (`ops.bin` unchanged).
- **Supersedes nothing;** it annotates the existing rigor ADRs (0006/0008/0015/
  0016/0018/0019) with the scope boundaries the review surfaced.
