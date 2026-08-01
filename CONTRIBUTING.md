# Contributing to Treease

Thank you for helping improve Treease. This repository is the public Treease
client and runtime repository; the hosted API implementation and its operational
configuration are maintained separately.

## Before opening a change

- Read the root `AGENTS.md` and the nearest scoped `AGENTS.md`.
- Keep public code independent of private service source paths.
- Do not commit `.env` files, credentials, provider keys, uploaded content, or
  deployment secrets.
- Put document parsing, formatting, operators, evaluation, and graph
  construction in `packages/core/`; keep Web presentation and interaction in
  `apps/web/`.
- For Desktop test changes, follow [`docs/testing/desktop-testing.md`](docs/testing/desktop-testing.md): Web/Core own shared behavior, while Desktop owns stable smoke paths and Desktop-specific boundaries.

## Local checks

Run the smallest checks relevant to the change. For a broad public-repository
change, run:

```bash
pnpm --dir apps/web check
pnpm --dir apps/web test:unit
cd packages/core && cargo nextest run --locked
cd ../../apps/cli && cargo nextest run --locked --lib
```

For documentation changes, also run:

```bash
pnpm docs:map:check
git diff --check
```

Generated protocol and WASM files must be regenerated through the documented
Core workflow; do not hand-edit generated protocol output.

## Pull requests

Describe the user-visible behavior, the ownership boundary affected, and the
checks you ran. Keep changes focused and remove obsolete paths rather than
adding compatibility shims without a documented contract.

Contributions must be original work or work you are authorized to submit. By
submitting a contribution, you agree that it may be distributed under the
repository license in `LICENSE`. The repository currently uses the Treease
Community License, which is source-available and not OSI-approved.

Please report suspected security issues privately rather than opening a public
issue with credentials or exploit details.
