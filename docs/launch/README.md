# Launch kit

This directory contains the public-facing material for Vantyx: accurate
descriptions, AI-discovery notes, launch drafts, and a checklist. The goal is
discoverability without exaggeration.

Published: https://github.com/zeiad6/vantyx · Release:
https://github.com/zeiad6/vantyx/releases/tag/v0.1.0

The repository was renamed from `zeiad6/auren`; the old URL redirects. CI run
45317367818 passed after the rename and AI foundation, and a clean clone passed
all 340 tests.

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

- [x] Full verification gate passes on the launch commit.
- [x] `PROJECT_STATUS.md` and `TASKS.md` match the code and test counts.
- [x] `LICENSE`, `CONTRIBUTING.md`, `SECURITY.md`, and `CODE_OF_CONDUCT.md` are present.
- [x] CI is green on the default branch.
- [x] The repository has a short description and topics.
- [x] `llms.txt` facts match `PROJECT_STATUS.md`.
- [x] The demo script runs from a clean checkout.
- [x] Benchmarks are cited with their date and methodology, not as a general claim.
- [x] Known limitations are visible in the README, not buried.
- [x] AI claims are limited to the offline host foundation and loopback tests.
- [x] GitHub repository renamed to `vantyx` and CI verified from a clean clone.

## Launch sequence

1. Publish the repository and confirm CI.
2. Create a `v0.1.0` tag and a release with the verified test counts and limits.
3. Post the demo and the honest limitations together.
4. Link the specification, status file, and benchmark methodology.
5. Respond to technical questions with reproducible commands.
6. Update `llms.txt` and the AI kit after the first external feedback.

## Honesty rules

- Never call Vantyx production-ready, stable, or 1.0.
- Never claim general performance superiority from six microbenchmarks.
- Never describe the native subset as "all Vantyx programs".
- Never hide the intentional native allocation leak or the missing cycle
  collector.
- Correct mistakes in public and update the source document in the same change.
