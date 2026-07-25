---
summary: "Public HTTP contract between the open Treease clients and the separately deployable service."
read_when:
  - Changing Web requests or responses served by the Hosted API.
  - Coordinating changes with the private service repository.
  - Adding a public API type, validation rule, or error code.
---

# API Boundary

## Ownership

`packages/api-contracts/` is the public source of truth for the HTTP boundary between
Treease clients and the service. It contains request schemas, client-visible response
schemas, and shared error shapes only. It must not import `apps/server/` or contain
billing-provider, database, provider-key, or repository models.

The private Hosted API owns authentication, persistence, billing, usage enforcement,
file storage, AI provider routing, and response assembly. Its internal service and
repository types are not client contracts.

`apps/web/` uses the public schemas to validate responses at the network boundary. It
must not import server source files or server-internal types. The API origin is a public
Web build setting (`PUBLIC_API_ORIGIN`), not a server secret.

```text
apps/web ───────> packages/api-contracts <────── Hosted API
     │                                             │
     └────────────── HTTPS JSON/multipart ─────────┘
```

## Contract rules

- Add or change client-visible fields in `packages/api-contracts/src/index.ts` first.
- Keep response shapes to the smallest client-needed public data. Do not expose provider
  IDs, database IDs, storage keys, usage ledger rows, or internal timestamps unless a
  client feature needs them.
- Validate request bodies on the Hosted API with the shared request schemas.
- Validate successful Web responses with the matching response schema; an invalid
  response is a protocol error, not an empty or partially trusted result.
- Keep `packages/share-protocol/` focused on serialized share resources. API envelopes,
  errors, billing summaries, and usage summaries belong in `api-contracts`.
- The private service may depend on a released `@treease/api-contracts` version after
  repository separation. The public client must never depend on a private repository
  path.

## Repository split gate

After the repository split:

1. The private service uses released versions of `@treease/api-contracts` and
   `@treease/share-protocol`, never a public-repository source path.
2. Web checks run without the private service checkout present.
3. The public history, environment examples, deployment manifests, and generated
   artifacts contain no secrets or private operational data.

License selection for the public packages is a legal/product decision and is not defined
by this contract.
