---
summary: "Implementation design for promoting a temporary share Tab into the local workspace on first mutation."
read_when:
  - Implementing or migrating share workspace promotion
  - Verifying the first-mutation rollout and cleanup plan
---

# Share Workspace Fork Implementation Design

## Background

Share restore already starts from a fresh one-Tab workspace, skips local session bootstrap and persistence, and restores the shared document through the existing whole-document path with usage metering disabled. The missing capability is to join that Tab to the user's persisted workspace when the first real draft mutation occurs.

Stable authority, lifecycle, mutation, promotion, persistence, error, and concurrency rules are owned by [Share Workspace Contract](./contracts/share-workspace.md). Primary-document semantics remain owned by [Editor Data Flow Contract](./contracts/editor-data-flow.md). This page records only implementation choices and migration work.

## Design Choices

### Page-level orchestration

The editor page owns the share lifecycle orchestrator because it already coordinates `WorkspaceHost`, workspace bootstrap, session persistence, Editor, and Graph adapters. The orchestrator calls narrow Modules without absorbing their Implementation.

### Pure topology transition

A pure share topology Module validates persisted session data, rebuilds recovered Tabs as idle drafts, appends the current share Tab, preserves its identity and runtime sidecar, and returns one complete workspace value. The page publishes that value once; recovered Tabs are never activated during merge.

### Separate mutation seams

Direct Monaco input notifies promotion after its synchronous model change and continues through the existing incremental commit. Commands that already wait for quota/planning/conversion await promotion immediately before their canonical model write. The common lifecycle is single-flight, but the two data flows keep their correct ordering.

### Persistence after authority publication

The first save and all retries serialize the published workspace through the common session mapper. Normal session subscription is attached only after one save succeeds and immediately schedules a latest projection, covering direct input that continued during the first save.

## Migration Steps

1. Add tests for session validation/projection and pure topology promotion.
2. Add the closed share lifecycle and single-flight persistence retry.
3. Configure the lifecycle from the editor page only for share URLs.
4. Notify it from direct Monaco draft changes.
5. Await it inside Graph apply, targeted replacement, and full-edit/import seams after existing gates.
6. Reuse the common workspace-to-session projection for bootstrap and persistence.
7. Remove temporary share booleans, duplicated session serialization, and any mutation-specific merge logic.
8. Verify Browser and Desktop through their shared Web workspace and `WorkspaceHost` adapters.

## Verification Plan

### Unit

- persisted Tab order/content and share identity survive merge;
- sidecar and runtime resources stay out of the session;
- invalid session and stale target return closed failures;
- concurrent promotion shares one flight;
- failed persistence retains topology and retries;
- Graph quota denial, stale/no-op format, and failed conversion do not promote.

### Integration

- share browsing performs no session I/O;
- direct input keeps the same model and all text;
- complete topology is published before save;
- the first subscribed save cannot overwrite merged topology;
- Graph and whole-document edits retain their canonical paths.

### End-to-end

- existing local Tabs are unchanged when an unmodified share closes;
- first edit appends one visible share Tab after the existing order;
- refresh restores existing Tabs plus the edited share Tab;
- rapid input promotes once;
- closing during promotion cannot resurrect the Tab;
- no file grant is restored from share/session data.

## Removal Gate

The migration is complete only when no second workspace store, session-to-workspace subscription, duplicate whole-document path, per-button share flag, compatibility facade, or fallback merge remains, and `pnpm check:circular` passes.
