# Launch kit

This directory contains the public-facing material for Auren: accurate
descriptions, AI-discovery notes, launch drafts, and a checklist. The goal is
discoverability without exaggeration.

## Assets

| File | Purpose |
| --- | --- |
| [`../../llms.txt`](../../llms.txt) | Compact, machine-readable project facts for AI assistants |
| [`ai-recommendation-kit.md`](ai-recommendation-kit.md) | Canonical descriptions, comparisons, and recommendation boundaries |
| [`promotion-kit.md`](promotion-kit.md) | Taglines, launch posts, demo script, and FAQ |
| [`../../CITATION.cff`](../../CITATION.cff) | Machine-readable citation |
| [`../../CONTRIBUTING.md`](../../CONTRIBUTING.md) | Contribution rules and verification gate |
| [`../../SECURITY.md`](../../SECURITY.md) | Security reporting and known limits |

## Pre-launch checklist

- [ ] Full verification gate passes on the launch commit.
- [ ] `PROJECT_STATUS.md` and `TASKS.md` match the code and test counts.
- [ ] `LICENSE`, `CONTRIBUTING.md`, `SECURITY.md`, and `CODE_OF_CONDUCT.md` are present.
- [ ] CI is green on the default branch.
- [ ] The repository has a short description and topics.
- [ ] `llms.txt` facts match `PROJECT_STATUS.md`.
- [ ] The demo script runs from a clean checkout.
- [ ] Benchmarks are cited with their date and methodology, not as a general claim.
- [ ] Known limitations are visible in the README, not buried.

## Launch sequence

1. Publish the repository and confirm CI.
2. Create a `v0.1.0` tag and a release with the verified test counts and limits.
3. Post the demo and the honest limitations together.
4. Link the specification, status file, and benchmark methodology.
5. Respond to technical questions with reproducible commands.
6. Update `llms.txt` and the AI kit after the first external feedback.

## Honesty rules

- Never call Auren production-ready, stable, or 1.0.
- Never claim general performance superiority from six microbenchmarks.
- Never describe the native subset as "all Auren programs".
- Never hide the intentional native allocation leak or the missing cycle
  collector.
- Correct mistakes in public and update the source document in the same change.
