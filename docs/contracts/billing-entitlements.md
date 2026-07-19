---
summary: "套餐权益、限额门控、使用账本与事件上报的唯一产品契约。"
read_when:
  - 设计或修改套餐、订阅、使用限额、功能门控或升级引导
  - 变更 Web 套餐展示、套餐分析事件或 Server 的订阅与用量模型
---

# Billing Entitlements

## Terms and authority

- **Plan tier** is the customer-facing entitlement: `free` or `pro`.
- **Billing cadence** is how a Pro subscription is billed: `monthly` or `yearly`; it is not a separate entitlement tier.
- **Entitlement snapshot** is the resolved, server-issued result for one authenticated user. The Web app may display it but must not calculate access from local state.
- **Usage event** is an immutable server-side record of one chargeable action. The ledger, not browser analytics, is the source for quota enforcement.

The server resolves subscriptions from verified Lemon Squeezy webhooks and owns the entitlement snapshot and usage ledger. AI access is server-enforced; local graph and file operations use the server-issued snapshot only to decide whether a computed result remains consumable.

Web and desktop create a Supabase anonymous user automatically when no session exists. Chargeable non-AI usage is recorded against that anonymous user's ID, and the first email or OAuth login links an identity to the same user instead of creating a new user. Anonymous users are therefore counted by the same usage ledger; the browser or desktop credential must be retained until the identity is linked. Supabase must enable anonymous sign-ins and manual identity linking for this flow.

## Pro product limits

| Capability | Free | Pro |
| --- | ---: | ---: |
| Bidirectional-edit documents per calendar month | 10 | Unlimited |
| Large-file visualization or processing runs per calendar month (source file ≥ 500 KB) | 3 | Unlimited |
| Share-link lifetime | 7 days | 365 days |
| AI suggestions per calendar month | 0 | 1,000 |
| Graph, source tracing, structural comparison, import/export | Included | Included |

`monthly` and `yearly` both resolve to **Pro**. A canceled or non-active subscription falls back to Free entitlements when its paid term ends; the webhook payload must supply the paid-through timestamp before that behavior is implemented.

## Enforcement path

```mermaid
flowchart LR
  subgraph Local["Graph edit / large-file processing"]
    W["Web action"] --> X["Compute and render result"]
    X --> L["Append consumed usage event asynchronously"]
    L --> S["Refresh server-issued usage summary"]
    S -->|"next action is over quota"| U["Compute result, then place pricing overlay over graph"]
  end
  subgraph AI["AI suggestion"]
    A["Protected API"] --> E["Resolve snapshot + atomic reservation"]
    E -->|"allowed"| P["Call provider and consume reservation"]
    E -->|"denied"| D["402 entitlement_exhausted"]
  end
```

`bidirectional_edit` and `large_file_processing` must not wait for a usage request before local computation. After a successful, consumable action, Web records a consumed event asynchronously. Once the server-issued summary says the limit is exhausted, the next local action still computes its graph result, but Web puts a pricing overlay over that result and blocks graph interaction until the entitlement changes. The masked preview is not another consumed usage event.

`ai_suggestion` remains server-enforced: authenticate, resolve the current entitlement, atomically reserve the quota, call the provider, then consume or release the reservation. Do not count browser clicks and do not add a browser-only source of entitlement truth.

## Target server model

`subscriptions` stores payment-provider state, while immutable tables make entitlement and quota decisions auditable.

```text
subscriptions
  user_id (unique), tier (free|pro), billing_cadence (monthly|yearly|null)
  status, current_period_end, provider_subscription_id, provider_variant_id
  last_provider_event_at, updated_at

entitlement_snapshots
  subscription_id, effective_from, effective_to, limits jsonb, features jsonb
  source (webhook|admin), created_at

usage_events
  id, user_id, capability, period_key, quantity, state (reserved|consumed|released)
  idempotency_key (unique), metadata jsonb, created_at, finalized_at
```

`limits` contains named numeric limits such as `ai_suggestions_monthly`; `features` contains explicit booleans such as `large_file_processing`. The entitlement resolver produces a closed result (`allowed`, `quota_exhausted`, `feature_unavailable`) so routes cannot combine nullable limits and booleans incorrectly. `usage_events` must have a unique `(user_id, period_key, idempotency_key)` index. Non-AI event recording is idempotent but deliberately permits a ledger total above the display limit; AI quota reservation must run in one database transaction or RPC to prevent concurrent provider calls from overspending a limit. Finalization must also bind the authenticated `user_id`, so one user can never consume or release another user's reservation.

## Web analytics contract

Analytics contains no email, user ID, document content, tree path, or provider identifier. It is for funnel diagnosis only and never controls access.

| Event | When | Parameters |
| --- | --- | --- |
| `subscription_viewed` | Signed-in user opens the account menu | `plan`, `status`, `surface` |
| `subscription_management_started` | User opens plan management | `plan`, `status`, `surface` |
| `entitlement_blocked` | A local graph result is masked because the server-issued summary is exhausted, or the AI API denies a gated capability | `plan`, `feature`, `reason`, `surface` |
| `quota_threshold_reached` | Server response indicates 80%, 100% usage | `plan`, `feature`, `threshold`, `surface` |

The first two events are implemented in `apps/web/src/lib/components/AccountMenu.svelte`. The latter two are added at the shared API-error/upgrade prompt boundary when those capabilities are introduced, not at each individual button.
