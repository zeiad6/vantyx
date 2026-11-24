# ADR-0014: Rename the language to Vantyx

- Status: Accepted
- Date: 2026-11-23
- Supersedes: ADR-0006 (name — Auren)

## Context

The project was published as **Auren** on 2026-11-23. The author asked for a
short, memorable, impactful name that is not a personal name and is tied to the
project's direction. The AI engineering directive names the project **Vantyx**
and explicitly asks for a rename audit.

The full evidence and availability checks are in
[`RESEARCH/NAMING_AUDIT.md`](../../RESEARCH/NAMING_AUDIT.md).

## Decision

Rename the language and toolchain to **Vantyx**.

| Item | Old | New |
| --- | --- | --- |
| Language name | Auren | Vantyx |
| CLI | `auren` | `vantyx` |
| Source extension | `.auren` | `.vtx` |
| Cargo package / library | `auren` | `vantyx` |
| C compiler override | `AUREN_CC` | `VANTYX_CC` |
| REPL history override | `AUREN_HISTORY` | `VANTYX_HISTORY` |
| Default history file | `~/.auren_history` | `~/.vantyx_history` |
| Repository | `zeiad6/auren` | `zeiad6/vantyx` |

## Rationale

- Six letters, two syllables, pronounceable, and not a person's name.
- No programming-language collision was found in GitHub, npm, PyPI, or
  crates.io checks on 2026-11-23. Unrelated companies and repositories exist;
  the name is not trademark-cleared.
- "Vantyx" evokes a vantage point over the whole stack: the same language
  scales from a Python-like script to a native C11 executable, and now toward
  AI-native engineering.
- The rename happens before 1.0, when migration cost is lowest.

## Compatibility

- GitHub redirects the old repository URL after the repository rename.
- The old name remains documented in ADR-0006 and the naming audit so existing
  citations and links remain understandable.
- The file extension changes. A one-release compatibility shim that also accepts
  `.auren` is desirable but is not part of this ADR; the CLI currently selects
  behavior by command, not by extension, so old files still run when passed
  explicitly. The documentation and examples use `.vtx`.
- Environment variables are not aliased. `VANTYX_CC` and `VANTYX_HISTORY` are
  the supported names; the old names are removed to avoid two sources of truth.

## Consequences

- Every source, test, example, benchmark, CI job, document, badge, and release
  note must use the new name in the same change.
- The public release `v0.1.0` remains the first release under the old name; the
  next release notes must state the rename.
- Package-registry names are not reserved yet.
- A trademark search is required before 1.0.

## Reconsideration criteria

Revisit before 1.0 if a trademark conflict appears, if the brand causes measurable
discoverability problems, or if the author prefers a descriptive name such as
Gradix or Natyx.
