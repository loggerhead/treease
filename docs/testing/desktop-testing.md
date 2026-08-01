---
summary: "Desktop test strategy, ownership, and maintenance rules."
read_when:
  - Writing or reviewing tests under `apps/desktop/`
  - Deciding whether a behavior belongs in Web tests or Desktop tests
  - Changing the Desktop test suite or its execution tiers
---

# Desktop Testing Guide

## Core Principles

Desktop and Web share the same workspace, editor semantics, and core logic.
Web tests are the primary source of truth for business behavior and should
provide the complete coverage of valid, invalid, boundary, and failure cases.
Desktop tests must not reproduce the Web test suite.

Desktop tests have two responsibilities:

1. Verify a small set of stable, high-value happy paths that must continue to
   work in the packaged Desktop environment. These tests should prove that a
   user can start the app, open the primary workflow, complete a representative
   operation such as incremental editing, save the result, and recover it.
2. Verify behavior that exists because the app is Desktop: the Tauri host,
   windows, IPC, file access, local persistence, system integrations, deep
   links, authentication handoff, updates, packaging, and platform-specific
   behavior.

Do not add a Desktop test merely because an equivalent Web test exists. Add it
only when the test exercises the Desktop boundary or protects a stable
end-to-end path whose failure would not be detected by Web tests.

For shared behavior, assert the user-visible result rather than implementation
details. Keep Desktop happy-path tests intentionally narrow; test an exception
on Desktop only when the exception is caused by a Desktop-specific boundary,
such as a denied file permission, a missing local file, an interrupted IPC
request, or an application restart.

## Test Ownership

| Behavior | Web tests | Desktop tests |
| --- | --- | --- |
| Core parsing, formatting, operators, evaluation, and graph construction | Full coverage | No duplication |
| Shared editor and workspace behavior | Full coverage, including failure cases | Stable smoke path only |
| Tauri host, windows, IPC, and native capabilities | Not applicable or limited | Primary coverage |
| Filesystem, persistence, deep links, auth handoff, updates, and packaging | Not applicable or limited | Primary coverage |
| Cross-platform behavior | Not applicable | Release-level coverage |

## Test Selection Rules

Before adding a Desktop test, answer these questions:

- Is the behavior Desktop-specific?
- If it is shared, is it a stable, high-value happy path?
- Is the behavior already covered by Web or Core tests?
- Can a failure be clearly attributed to the Desktop boundary?
- Is the long-term maintenance cost justified by the risk it protects?

Prefer tests that are deterministic, short, and resilient to visual or copy
changes. Avoid testing incidental layout, volatile text, or internal component
structure unless those are themselves a Desktop contract.

## Maintenance Rules

- Keep Web tests as the canonical coverage for shared business behavior.
- Keep Desktop tests limited to stable happy paths and Desktop-specific risks.
- Label each Desktop test by its purpose: stable workflow or Desktop-specific
  behavior.
- When shared logic changes, update Web or Core tests first; update Desktop
  tests only if the Desktop boundary or the selected smoke path changes.
- When windows, IPC, filesystem access, persistence, authentication handoff,
  updates, packaging, or platform behavior changes, review the relevant
  Desktop tests.
- Delete Desktop tests that duplicate Web coverage or protect removed behavior.
- Avoid assertions on implementation details that make harmless Web or Desktop
  refactors require test changes.
- Keep the fast policy and smoke checks separate from release-level platform
  checks so local feedback remains quick.
- A pull request that changes Desktop behavior must state which Desktop test
  tier was run and why any untested path is owned by Web or Core tests.

## Local Verification

Run the smallest relevant check:

```bash
pnpm --dir apps/desktop test:policy
pnpm --dir apps/desktop test:e2e
```

`test:policy` protects static Desktop security and release configuration.
`test:e2e` is reserved for the stable workflow and Desktop-specific integration
coverage; it should not become a second copy of the Web suite.
