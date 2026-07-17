---
summary: "Documentation ownership, discovery, and generated-artifact rules."
read_when:
  - Writing, moving, or deleting repository documentation
  - Selecting the minimum documentation packet for a code or configuration change
---

# Docs Guide

## Purpose

Documentation helps an agent make one change with the smallest sufficient context. Put each fact in one owner; link to that owner instead of copying it elsewhere.

## Ownership

- `docs/contracts/` owns stable terms, authority, state transitions, and invariants.
- Topic directories own focused domain guidance and user-facing references.
- `docs/generated/`, `docs/docs_map.md`, and root generated snapshots report current state only. They never define product or implementation semantics.
- `AGENTS.md` files own scoped development contracts: placement, interfaces, seams, local proof, and any task-specific extra reading.

## Discovery and Metadata

- `summary` and `read_when` belong only on hand-written, agent-discoverable guidance pages. Keep `read_when` to one to three concrete triggers.
- Do not add frontmatter to `AGENTS.md`, `README*`, `SKILL.md`, operator reference leaves, fixtures, scratch files, or generated artifacts.
- `pnpm docs:list` is the task-discovery index. It intentionally excludes `docs/operators/`, generated artifacts, and `docs/docs_map.md`; open those paths directly only when the task requires them.
- `docs/docs_map.md` is the generated heading map. It excludes operator references; regenerate it with `pnpm docs:map:gen` rather than editing it.
- Use repository-relative paths. Never place local identity data, absolute local paths, or secrets in documentation.

## Reading Rules

- Start with the root or nearest scoped `AGENTS.md`.
- Add the relevant contract.
- Read implementation sources and run the smallest relevant verification from the root `AGENTS.md`.
