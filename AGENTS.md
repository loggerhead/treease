# AGENTS.MD

Telegraph style. Root rules only. Read scoped `AGENTS.md` before subtree work. root owns hard policy and task routing.

## Start

- Replies: repo-root refs only: `apps/web/svelte.config.js`. No absolute paths, no `~/`.
- Docs/user-visible work: `pnpm docs:list`, then read relevant docs only.
- Existing-solutions preflight: before proposing or building a custom system, feature, workflow, tool, integration, or automation, do a lightweight check for open-source projects, maintained libraries, or free platforms that already solve it well enough. Prefer those when adequate. Build custom only when existing options are unsuitable, too expensive, unmaintained, unsafe, non-compliant, or the user explicitly asks for custom. Avoid paid-service recommendations unless the user explicitly approves spend. Keep this to a brief preflight gate, not a broad research assignment.
- Fix/triage answers need source, unit tests, current/shipped behavior, and dependency contract proof.
- Review default: read the whole changed function/module plus callers, callees, sibling implementations, adjacent tests, scoped docs, and dependency/Codex contracts before saying `good`, `bad`, `best fix`, `proof sufficient`, or posting a comment. If challenged, keep reading first; do not defend the earlier verdict until the missing path is checked.
- Dependency-touching work: direct dependency inspection is mandatory when feasible; do not rely on assumptions, wrappers, or memory. Most dependencies are OSS, so read their source/docs/types.
- Dependency-backed behavior: read upstream docs/source/types first. No API/default/error/timing guesses.
- External API work: live test required. Google/search for additional proof. Prefer official docs/source/types; cite current proof. No memory-only API claims.
- Live-verify when feasible. Never print secrets or private data.

## Map

- `packages/core/`: Rust parsing, formatting, operators, evaluation, and graph construction.
- `apps/web/`: editor and graph UI.
- `apps/server/`: accounts, billing, sharing, and AI server capabilities.
- `apps/cli/`: standalone CLI crate, acceptance tests, and documentation entry points.
- Docs: read `docs/AGENTS.md`; use `pnpm docs:list` to select the minimum relevant guide in `docs/contracts/`.

## Architecture

- Fix shape: default to a clean bounded refactor, not the smallest patch. Move ownership to the right boundary; delete stale abstractions, duplicate policy, dead branches, wrappers, and fallback stacks.
- Refactor default: one canonical path. Delete the old path unless the user explicitly wants compatibility or a public contract supports it.
- Fallback is a product decision, not an implementation convenience. Before adding one, name the formal contract, failure mode, and removal plan. Otherwise delete it.
- Do not fix logic or bugs with fallbacks, patch branches, silent degradation, dual-write semantics, or a special case for the current failure. Fix the main path, protocol source of truth, or real ownership boundary.
- If unsure whether compatibility is needed, ask first. Do not keep aliases, shims, fallbacks, stale names, or obsolete tests just in case.
- Tests alone do not make internals contracts. If compatibility stays, name the contract and migration/removal plan in code, tests, or PR documentation.
- Lean code is a goal. No internal shims, aliases, legacy names, broad fallbacks, or defensive branches merely to reduce a diff or handle hypothetical edge cases.
- Inline comments preserve reviewer context at the code site. Required for cross-module state invariants, execution order, ownership boundaries, resource-release coupling, fallback behavior, compact encoding, or intentional caller differences.
- Comment shape: 1-3 short lines; state why the branch/helper exists, what contract it protects, and the bad outcome if removed. Cite nearby constants when useful. No syntax narration, PR history, or obvious mechanics.
- No cross-layer shortcuts: Web does not import `packages/core/src`; Core does not own Svelte, DOM, or browser logic; CLI and desktop do not copy Core implementations.
- Web owns presentation, interaction, and frontend state only. Parsing, formatting, operators, evaluation, and graph construction belong in Core.
- Docs, comments, example commands, and screenshot annotations must not contain local identity data, absolute paths, or environment variable values.
- Web async commits follow existing `FreshnessScope`/guard semantics. Discard stale results; never overwrite current UI state.
- Cross-component shared state goes through existing stores; do not couple non-parent/child components directly.
- Protocol source of truth: `packages/core/src/document/protocol.rs`. Never hand-edit `packages/core/wasm/document-protocol.generated.ts`.

## Code

- TS ESM, strict. Avoid `any`; prefer real types, `unknown`, and narrow adapters.
- No `@ts-nocheck`. Disable checks only after careful consideration and with an explanatory comment.
- External boundaries: prefer `zod` or existing schema helpers.
- Runtime branching: discriminated unions/enums over freeform strings. Avoid semantic sentinels (`?? 0`, empty object/string).
- Cross-function state: when valid combinations matter, return a closed mode/result shape. Avoid parallel nullable fields or derived booleans that callers must keep in sync; make impossible states unrepresentable.
- Calls should be boring: complex decisions happen above; call args/object fields are names, literals, or simple property reads.
- Prefer early returns over nested condition pyramids. Split code into gather -> normalize -> decide -> act.
- Use named intermediates only for domain meaning or readability; avoid temporary-variable soup.
- Code size matters. Prefer small clear code; maintainability includes not growing LOC without payoff.
- Refactors should delete about as much local complexity as they add. If LOC grows, the new ownership/API needs to clearly pay for it.
- Refactors should reduce non-test LOC unless they remove a larger architectural cost. Treat positive production LOC as a smell. Before closeout, run `git diff --numstat`; if non-test LOC grew, trim or explicitly explain how many paths were removed.
- Prefer deleting branches, modes, adapters, and tests over preserving them. A refactor that adds a second path has probably failed unless the old path is a cited shipped contract.
- New helpers/files must pay rent immediately: fewer call paths, fewer concepts, or less repeated logic. No helpers for one-off compatibility, field-name translation, or speculative resilience.
- Before adding helpers/files, check whether existing code can absorb the behavior with less new surface.
- Keep APIs narrow: export only current caller needs; keep types/helpers local by default.
- Return the smallest useful shape. Avoid broad result objects, flags, or metadata unless callers use them.
- Avoid adapter layers that only rename fields. Move real responsibility or leave code local.
- Inline simple one-use objects/spreads when clearer. Extract only when it removes duplication or hard logic.
- Tests prove behavior and regressions, not every internal branch.
- Tests are welcome, but review them before landing for duplication and value. Delete weak tests and assertions for behavior or paths just removed.
- Tests protect canonical behavior and migration boundaries, not obsolete internals. Delete tests for removed fallback paths instead of updating them.
- Prefer existing narrow helpers over repeated casts/guards. Add local helpers when two or more nearby call sites share real boundary logic.
- Prefer constructor parameter properties for injected dependencies/configuration. Do not ban them for erasable-syntax purity.
- Prefer `satisfies` for registries/config maps; derive types from schemas when a runtime schema already exists.
- Table-drive repetitive tests when it reduces code and keeps failure names clear.
- Dynamic import: no static and dynamic import for the same production module. Use `*.runtime.ts` as the lazy boundary. After edits, run `pnpm build` and check `[INEFFECTIVE_DYNAMIC_IMPORT]`.
- Web cycles: `pnpm check:circular` must pass.
- Classes: no prototype mixins or mutations. Prefer inheritance/composition. Tests prefer per-instance stubs.
- Split files around 700 LOC when clarity and testability improve.

## Commit Rules

- Commit messages are English and follow `type(scope): summary`.
- Changes that require `crate publish` must bump the affected package version in the same commit.
- When publishing `treease-core` after changing `packages/core`, update `packages/core/Cargo.toml`.
- When publishing `treease-cli` after changing `apps/cli`, update `apps/cli/Cargo.toml`.

## Verification

- Select the smallest relevant check; do not run everything indiscriminately.
- Core: `cd packages/core && cargo nextest run --locked`
- CLI: `cd apps/cli && cargo nextest run --locked --lib`; `cd apps/cli && bash tests/acceptance/run.sh`
- Web: `cd apps/web && pnpm test:unit` / `pnpm test:integration` / `pnpm test:e2e`
- Server: `cd apps/server && node --import tsx --test src/**/*.test.ts`
- Protocol or WASM changes: `cd packages/core && cargo run --locked --bin export_document_protocol`, then `cd apps/web && pnpm wasm:bindgen`; run `pnpm wasm:sync` when needed.
