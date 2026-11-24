# Naming Audit — Auren → Vantyx

Date: 2026-11-23
Decision owner: project author
Status: **Adopted** (directive names Vantyx; execution tracked as T-036)

## Brief

The author asked for a project name that is:

- related to the project's content,
- short,
- memorable and attractive,
- impactful,
- not a person's name.

The previous name, **Auren**, is short and pronounceable but reads like a
personal name and has no direct link to gradual typing or native compilation.
The AI directive names the project **Vantyx** throughout and asks for a rename
audit. This document records the evidence and the decision.

## Candidate evaluation

| Name | Length | Content link | Distinctiveness | Verdict |
| --- | ---: | --- | --- | --- |
| **Vantyx** | 6 | Brand-first: "vantage" (a higher viewpoint) + a technical suffix; links to seeing both the script and the native machine | No programming-language collision found; unrelated companies/repos exist | **Adopted** |
| Gradix | 6 | Gradual + native | No package found at audit time | Strong alternative if a descriptive name is preferred |
| Natyx | 5 | Native + syntax | No package found at audit time | Strong alternative, narrower |
| Typra | 5 | Typing | No package found at audit time | Descriptive but generic |
| Auren | 5 | None; sounds personal | Existing unrelated uses | Rejected by the author |

## Availability checks (observed 2026-11-23)

| Source | Query | Result |
| --- | --- | --- |
| GitHub repository search | `vantyx` | 85 repositories; none is a programming language |
| GitHub repository search | `vantyx language` | 1 repository; an NLP API project, not a language |
| npm registry | `vantyx` | HTTP 404 (no package) |
| PyPI JSON API | `vantyx` | HTTP 404 (no package) |
| crates.io API | `vantyx` | HTTP 404 (no crate) |
| GitHub repository search | `auren` | Existing unrelated projects; the public `zeiad6/auren` repository is this project |

These checks are not trademark clearance. They only show that the exact name
was not occupied in the checked registries at that moment. Unrelated uses found:
`Vantyx Group`, `VantyxBot`, `VantyxMedia`, and several small repositories.

## Decision

Adopt **Vantyx**.

- Language: **Vantyx**
- CLI: `vantyx`
- Source extension: `.vtx`
- Crate and library: `vantyx`
- Environment prefix: `VANTYX_` (for example `VANTYX_CC`, `VANTYX_HISTORY`)
- Repository: `https://github.com/zeiad6/vantyx`
- Tagline: **"Script like Python. Ship native. Build AI-native."**

The content link is carried by the tagline and the two-track identity:

1. **Vantage over the whole stack** — one language from a Python-like script to
   a native C11 executable.
2. **AI-native engineering** — the same language and toolchain should make
   local and cloud AI applications first-class, without hiding resource use or
   security boundaries.

This is a brand name, not a descriptive one. The documentation must state what
the project does; the name alone does not.

## Risks and mitigations

| Risk | Severity | Mitigation |
| --- | --- | --- |
| Unrelated `Vantyx` companies and repositories | Medium | Use the `zeiad6/vantyx` namespace; perform a trademark search before 1.0 |
| Rename breaks links, scripts, and the published release | High | GitHub redirects the old repository; keep a migration note; update badges, CI, docs, and examples in one change |
| File extension change breaks existing user files | Medium | Provide a migration command or accept both extensions for one release; document the change |
| Name is not descriptive | Low | Always pair it with the tagline and a one-sentence description |
| Package-registry squatting | Medium | Publish the crate and reserve npm/PyPI names when the release process allows |

## Execution plan (T-036)

1. Rename Cargo package, library, binary, extension, environment variables, and
   history path.
2. Update source imports, tests, examples, benchmarks, tools, CI, and docs.
3. Create ADR-0014, superseding ADR-0006 without deleting the original.
4. Rename the GitHub repository and update the remote, description, topics,
   release notes, and pinned issue.
5. Verify from a clean clone: build, 241+ tests, clippy, examples, and native
   differential output.
6. Keep `auren` as a documented former name so existing links and citations
   remain understandable.

## Revisit criteria

Before a 1.0 release:

- run a proper trademark search in the relevant jurisdictions,
- confirm crate/npm/PyPI availability,
- check search-result ambiguity for "Vantyx programming language",
- reconsider a descriptive name only if evidence shows the brand is a barrier.
