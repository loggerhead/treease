---
summary: "Share workspace authority, first-draft-mutation promotion, session persistence, and lifecycle invariants."
read_when:
  - Changing share restore, first-mutation behavior, or share workspace promotion
  - Changing workspace session persistence for a share URL
---

# Share Workspace Contract

## Scope

A share link represents one browsable document snapshot, not the sharer's complete workspace. This contract owns the stable authority, lifecycle, promotion, persistence, and mutation-entry rules for opening that snapshot and turning it into a local workspace tab.

Primary-document semantics remain owned by [Editor Data Flow Contract](./editor-data-flow.md). This contract intentionally records the stable promotion authority and lifecycle, not its historical implementation sequence.

## Authority

- `EditorWorkspaceState` is the only writable authority for left-Tab topology.
- The resident `Editor Model` is the active draft-text authority.
- `Commit Transaction` is the only primary-document commit entry.
- `Document Runtime` exclusively owns snapshot, parse, and Graph semantics.
- `WorkspaceSession` is a persisted projection of `EditorWorkspaceState`, never a second live topology.
- Browser and Desktop session I/O passes only through `WorkspaceHost`.
- The share lifecycle owns only restore eligibility, promotion single-flight, and persistence-retry state. It does not own Tab topology, a Monaco model, document identity, quota policy, or document semantics.

## Lifecycle

```text
restoring
  ├─ success → ephemeral-ready
  └─ failure → restore-failed

ephemeral-ready
  ├─ browse → ephemeral-ready
  ├─ close → disposed
  └─ first real draft mutation → promoting

promoting
  ├─ topology published → promoted-pending-persist
  └─ load / validation / merge / stale-target failure → promotion-failed

promotion-failed
  ├─ next valid mutation / explicit retry → promoting
  └─ close → disposed

promoted-pending-persist
  ├─ save success → promoted
  └─ bounded retry → promoted-pending-persist

promoted → ordinary workspace lifecycle
```

The lifecycle is a discriminated state. Multiple booleans, nullable Promises, or independently derived flags must not represent these phases.

## Share Restore

Share restore:

1. starts from one fresh temporary left Tab;
2. does not call `WorkspaceHost.loadSession()`;
3. does not enable the session-save subscription;
4. uses the canonical targeted whole-document replacement path;
5. explicitly skips usage metering;
6. restores only the share protocol's portable interaction state;
7. enters `ephemeral-ready` only after restore succeeds.

Restore, parsing, Graph construction, selection, focus, viewport, reveal, navigation, view-mode changes, compare restoration, and no-op commands never promote the workspace.

## First Draft Mutation

A successful first mutation means either:

- new draft text has already entered the target `Editor Model`; or
- a quota-approved, current, non-no-op mutation has produced its result and is about to enter the target `Editor Model` through the canonical path.

It does not require `SnapshotReady`. `ParseFailed` remains a successful draft mutation when the model text changed.

### Direct Editor input

```text
User input
→ Editor Model changes synchronously
├→ existing Commit Transaction
└→ single-flight share promotion
```

Promotion captures the triggering `tabId`, `documentKey`, resident model, and current model text. Later direct input continues in that same model and joins the in-flight promotion. `Commit Transaction` does not depend on `WorkspaceHost`, the share lifecycle, or session persistence.

### Command mutations

```text
capture stable target
→ quota gate when applicable
→ planner / conversion / format
→ current and non-no-op result
→ ensure share workspace promoted
→ canonical Editor Model mutation
→ Commit Transaction
→ Document Runtime
```

Quota denial, planner or conversion failure, stale/cancelled results, and no-op results do not promote. Graph writes still pass through `Graph Edit Planner`; whole-document writes still use the canonical targeted replacement Module.

## Canonical Topology Promotion

Promotion uses one narrow pure transition. It accepts immutable workspace/session data plus the captured share target and returns a complete `EditorWorkspaceState` or a closed rejection result. It does not call Svelte, Monaco, Worker, Document Runtime, or `WorkspaceHost`, and it does not save a session.

The result must:

1. preserve every valid persisted left Tab in order, including name, language, text, origin, and saved text;
2. append the share Tab exactly once and make it active, primary, and the left-pane Tab;
3. preserve the share `tabId`, `documentKey`, resident model, and latest captured model text;
4. publish the complete topology once, without activating recovered Tabs or switching the visible model;
5. keep the current share sidecar in runtime topology but out of left `tabOrder` and `WorkspaceSession.tabs`;
6. restore no file-access grant;
7. retain snapshot bindings only for retained runtime documents;
8. restore all recovered operation UI state as idle;
9. keep Promise, Job, generation, queue, reader, and other runtime resources out of the session.

An invalid persisted session is a promotion failure. It must not be replaced with an empty session that silently discards old Tabs.

## Persistence

The only persistence direction is:

```text
EditorWorkspaceState
→ WorkspaceSession mapper
→ WorkspaceHost.saveSession()
```

The required ordering is:

1. load persisted session;
2. build the complete promoted topology;
3. publish `EditorWorkspaceState` once;
4. verify the active model still matches the share identity;
5. project and save `WorkspaceSession` from the published authority;
6. attach normal session persistence.

If the first save fails after topology publication, do not roll back the draft or topology. Enter `promoted-pending-persist`, report that local recovery data is unsaved, and perform bounded cancellable retries. Attaching persistence must immediately project the latest workspace once so edits made during the first save are not lost.

This contract guarantees single-flight and idempotence only within the current application instance. It does not claim cross-window compare-and-swap behavior absent from `WorkspaceHost`.

## Concurrency and Cleanup

- Concurrent mutations share one promotion flight and append at most one share Tab.
- Promoted topology is published at most once and persistence is attached at most once.
- Closing the target synchronously invalidates an unpublished promotion; late load/merge results only clean up.
- Closing after topology publication follows the ordinary Tab-close contract.
- Restore and promotion never run concurrently.
- Retry cancellation, cleanup, and unsubscribe are idempotent.

## Error Classes

Callers distinguish share resource load/validation failure, restore failure, quota denial, mutation no-op, stale/cancelled mutation, persisted-session validation failure, topology rejection, target close, and session-save failure.

Fallbacks must not hide these failures. In particular, implementations must not discard old Tabs on load failure, create a replacement share Tab after promotion failure, claim persistence succeeded after save failure, or bypass the canonical document path.

## Verification Checklist

- Does opening and browsing a share avoid session load/save?
- Does the first direct edit preserve every character and the resident model?
- Do command mutations promote only after quota/result/freshness/no-op checks?
- Does one pure transition publish the complete topology before any save?
- Is the session always projected from the current workspace authority?
- Can close, retry, and concurrent mutation paths append at most one share Tab?
- Do Graph Planner, Commit Transaction, Document Runtime, and WorkspaceHost retain their existing responsibilities?
