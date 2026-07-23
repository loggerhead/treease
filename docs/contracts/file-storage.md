---
summary: "Unified inline and Cloudflare R2 file storage contract for shares and feedback attachments."
read_when:
  - Adding or changing share resources or feedback attachments.
  - Changing file upload, file download, retention, or cleanup behavior.
  - Changing the Cloudflare Worker, R2 binding, or Supabase metadata schema for user files.
---

# User File Storage Contract

## Decision

Treease uses one file-storage boundary for share resources and feedback attachments:

```text
ShareService / FeedbackService
            |
            v
    FileStorageService
            |
      +-----+-----+
      |           |
   inline       Cloudflare R2
   in DB        large objects
```

Files smaller than the inline threshold are stored in Supabase Postgres. Larger files are stored in a private Cloudflare R2 bucket, while Supabase stores their metadata and R2 key.

The storage decision, read path, deletion path, and integrity checks belong to `FileStorageService`. Share and feedback services own only their business records and access policies.

This is a new canonical design. Compatibility with the current `share_links.resource_payload` shape and the current external feedback endpoint is not required.

## Goals

- Store small structured share resources without an R2 round trip.
- Store large share resources and user-submitted feedback files in R2.
- Let share and feedback read either storage mode through the same service.
- Keep R2 private; no browser-held R2 credentials and no public bucket.
- Make expiration and deletion idempotent across Supabase and R2.
- Preserve enough metadata to validate size, content type, checksum, and cleanup state.
- Keep the implementation owned by the existing `apps/server` Cloudflare Worker.

## Non-goals

- Do not keep a compatibility reader for the old `resource_payload` column.
- Do not use Supabase Storage for these files.
- Do not expose a generic public file URL.
- Do not place the same cleanup responsibility in both Supabase Cron and Cloudflare Cron.
- Do not copy the entire contents of large files into GitHub Issue bodies.

## Storage boundary

### FileStorageService

The service must provide a narrow API similar to:

```ts
type CreateFileInput = {
  body: Uint8Array | ReadableStream<Uint8Array>;
  byteSize: number;
  contentType: string;
  fileName?: string;
  expiresAt: string;
};

type StoredFile = {
  id: string;
  storageKind: 'inline' | 'r2';
  contentType: string;
  fileName: string | null;
  byteSize: number;
  sha256: string;
  expiresAt: string;
};

interface FileStorageService {
  create(input: CreateFileInput): Promise<StoredFile>;
  read(id: string): Promise<{ metadata: StoredFile; body: Uint8Array }>;
  open(id: string): Promise<{ metadata: StoredFile; body: ReadableStream }>
  delete(id: string): Promise<void>;
}
```

`FileStorageService` must:

1. Validate the declared size against the actual size when the body is buffered or finalized.
2. Compute SHA-256 from the bytes that are stored.
3. Select storage using the original byte size, not character count or base64 length.
4. Return `file_not_found` for missing records or missing R2 objects.
5. Return `file_expired` for expired records before opening content.
6. Treat deletion of an already-deleted R2 object as success.
7. Never return R2 credentials or an unrestricted object key to the browser.

### Inline threshold

The initial threshold is:

```ts
const INLINE_FILE_MAX_BYTES = 256 * 1024;
```

This is a server-owned constant. It is not a billing entitlement and must not be configured independently by Web and Server.

The threshold applies to the original bytes:

- `ShareResource`: UTF-8 bytes of the canonical serialized resource envelope.
- Feedback attachment: original uploaded bytes.

Files at or above the threshold use R2.

## Database model

### `file_objects`

Create a dedicated table in the Server Supabase migrations:

```sql
create table public.file_objects (
  id uuid primary key default gen_random_uuid(),
  storage_kind text not null check (storage_kind in ('inline', 'r2')),
  lifecycle_state text not null default 'pending'
    check (lifecycle_state in ('pending', 'ready', 'deleting', 'deleted')),
  content_type text not null,
  file_name text,
  byte_size bigint not null check (byte_size >= 0),
  sha256 text not null,
  inline_bytes bytea,
  r2_key text,
  created_at timestamptz not null default now(),
  expires_at timestamptz not null,
  delete_attempts integer not null default 0,
  last_delete_error text,
  deleted_at timestamptz,
  check (
    (lifecycle_state = 'deleted' and inline_bytes is null and r2_key is null)
    or
    (lifecycle_state <> 'deleted' and storage_kind = 'inline' and inline_bytes is not null and r2_key is null)
    or
    (lifecycle_state <> 'deleted' and storage_kind = 'r2' and inline_bytes is null and r2_key is not null)
  )
);

create index file_objects_expiry_idx
  on public.file_objects (expires_at, lifecycle_state);

alter table public.file_objects enable row level security;
```

The table is accessed by the Server with the Supabase service role. Do not expose it to the browser Data API. If the project Data API exposes the table, add explicit grants and policies that still prevent arbitrary reads and writes.

`r2_key` must be generated from the opaque file UUID, for example:

```text
files/{file_id}
```

Do not place email addresses, user IDs, local paths, or original filenames in the R2 key.

### Share records

Replace the current inline payload column with a file reference:

```sql
alter table public.share_links
  add column file_id uuid not null references public.file_objects(id);
```

Then remove `resource_payload` after the new path is implemented. Keep `resource_type` as an indexed business discriminator, even though the stored file contains the complete resource envelope.

The stored share file contains the canonical serialized form of:

```ts
{
  type: resourceType,
  payload: resourcePayload,
}
```

### Feedback records

Feedback must be owned by the Treease Server if it is to share this storage boundary. Add:

```sql
create table public.feedback_submissions (
  id uuid primary key default gen_random_uuid(),
  user_id uuid,
  email text,
  category text not null check (category in ('bug', 'feature', 'question')),
  description text not null,
  github_issue_url text,
  created_at timestamptz not null default now(),
  expires_at timestamptz not null
);

create table public.feedback_attachments (
  feedback_id uuid not null references public.feedback_submissions(id) on delete cascade,
  file_id uuid not null references public.file_objects(id) on delete restrict,
  role text not null check (role in ('data_file', 'screenshot', 'console_logs')),
  primary key (feedback_id, file_id)
);

create index feedback_submissions_expiry_idx
  on public.feedback_submissions (expires_at);

alter table public.feedback_submissions enable row level security;
alter table public.feedback_attachments enable row level security;
```

Feedback attachments should have a shorter default retention than share links. The initial default is 30 days. The service must assign the explicit `expires_at`; cleanup must never infer it from the file type.

## Write flows

### Share creation

1. `ShareService` validates the requested share lifetime and calculates `expiresAt`.
2. It serializes the resource envelope to UTF-8 bytes.
3. It calls `FileStorageService.create` with the same `expiresAt`.
4. It inserts `share_links.file_id` and the resource metadata.
5. If the database insert fails after R2 creation, the file remains `pending` or is explicitly deleted; cleanup must reclaim it.

### Feedback submission

1. Web submits feedback metadata and attachments to a first-party Server endpoint.
2. The Server validates category, email, file name, MIME type, and size limits.
3. Each attachment goes through `FileStorageService.create`.
4. The Server inserts `feedback_submissions` and `feedback_attachments`.
5. The Server creates the GitHub Issue after the records are durable.
6. The Issue body contains the feedback description, contact email, attachment names, sizes, and internal identifiers. It does not contain an unbounded file body.

The current Web direct call to `https://feedback.treease.com/api/feedback` must not remain the canonical path. Either move that endpoint into `apps/server` or update the BugDrop service to use the same file-storage implementation and the same R2 bucket. The preferred path is a new Server route such as `POST /v1/feedback`.

## Read flows

### Public share

`GET /v1/public/shares/:shareID` remains the public business endpoint.

1. `ShareService` loads the share row.
2. It checks the share expiration.
3. It calls `FileStorageService.read(fileId)`.
4. It parses the bytes with `shareResourceSchema`.
5. It returns the existing resource envelope to Web.

The caller must not need to know whether the file was inline or in R2.

### Feedback attachment

Feedback attachments are not public. The Server should provide an authenticated operator path, or consume the file internally while creating an Issue:

```text
FeedbackService → FileStorageService.open(fileId) → R2 stream or inline stream
```

An attachment download endpoint must authorize the operator or owning user before calling `open`. A `file_id` alone is not an authorization credential.

## Cloudflare deployment

### R2 binding

Add one private bucket to the Server Worker configuration:

```jsonc
{
  "r2_buckets": [
    {
      "binding": "USER_FILES",
      "bucket_name": "treease-user-files"
    }
  ]
}
```

Keep this separate from the existing static-asset R2 bucket. User files have different retention and access rules.

R2 access must use the Worker binding. Do not expose S3 API credentials to Web. Use opaque R2 keys and keep the bucket private.

### Runtime environment seam

The current `apps/server/src/env.ts` validates string configuration with Zod. `R2Bucket` is a Worker binding, not a string environment variable. Keep these concepts separate:

```ts
type AppConfig = z.infer<typeof envSchema>;

type WorkerBindings = {
  USER_FILES: R2Bucket;
};

type RuntimeEnv = AppConfig & WorkerBindings;
```

`createAppServices` should receive the runtime storage binding explicitly instead of trying to parse it through `readEnv`.

## Cleanup deployment

Cleanup belongs in the existing Cloudflare Worker scheduled handler, not in a second application:

```ts
export default {
  fetch: workerApp.fetch,

  async scheduled(controller, env, ctx) {
    await cleanupExpiredFiles(env);
  },
};
```

Configure one hourly trigger:

```jsonc
{
  "triggers": {
    "crons": ["0 * * * *"]
  }
}
```

The scheduled handler is the only owner of cross-system file deletion. It has access to both Supabase service credentials and the `USER_FILES` R2 binding.

### Cleanup algorithm

Process a bounded batch, for example 100 records per invocation:

```text
1. Select file_objects where:
   expires_at <= now()
   and lifecycle_state = 'ready'
   limit 100

2. Mark each selected record as 'deleting'.

3. For storage_kind = 'r2':
   delete USER_FILES.delete(r2_key).

4. For storage_kind = 'inline':
   clear inline_bytes.

5. Mark successful records as 'deleted', set deleted_at,
   and clear r2_key / inline_bytes.

6. Increment delete_attempts and retain last_delete_error for failures.
```

The selection/update must tolerate overlapping or repeated invocations. If row locking is used, lock only the current batch and skip already `deleting` rows. If a Worker invocation fails after marking rows, the next run must reclaim rows stuck in `deleting` after a recovery window, such as one hour.

### Orphan cleanup

There are two cross-system failure windows:

- R2 upload succeeds, but the Supabase metadata update fails.
- Supabase metadata is removed, but R2 deletion fails.

Handle them as follows:

- Use `pending` for uploads not yet finalized.
- Reclaim stale `pending` records and their R2 keys.
- Keep deletion idempotent.
- Configure a conservative R2 lifecycle rule as a final safety net, for example deleting objects under `files/` after 400 days.

The lifecycle rule is only a safety net. It cannot implement per-record share or feedback retention because R2 does not read `expires_at` from Supabase.

R2 supports lifecycle rules and object deletion through the Worker binding. The Worker cleanup remains the source of truth for exact retention.

## Supabase responsibilities

Supabase stores:

- `file_objects` metadata and lifecycle state.
- Share and feedback business records.
- File associations.
- Cleanup attempt information.

Supabase does not directly delete R2 objects.

Supabase Cron may be used only for database-local maintenance, such as identifying stale `pending` rows or reporting cleanup failures. Do not create a second job that also deletes `file_objects` or calls R2. Supabase Cron supports SQL, database functions, and HTTP calls, but making it the R2 deletion owner would introduce an unnecessary cross-service trigger and a second retry path.

All new public-schema tables must have RLS enabled. The Server uses the service role for its repository operations; the browser must not have direct access to `file_objects`, `feedback_submissions`, or `feedback_attachments`.

## API shape

### Share API

Keep the public response contract:

```text
POST /v1/share-links
GET  /v1/public/shares/:shareID
```

Only the server-side implementation changes from `resource_payload` to `file_id`.

### Feedback API

Add a first-party route:

```text
POST /v1/feedback
```

The request must support metadata plus binary attachments. Do not base64-encode large files into JSON. Use a multipart request or a dedicated file-upload phase.

For an initial implementation with a bounded feedback file limit, the Server can receive the request body and route it through `FileStorageService`. If larger uploads are later needed, add a presigned R2 upload phase only for files whose declared size is above the inline threshold, followed by a server-side completion call.

The client must never choose `storage_kind`; the Server decides it from the measured file size.

## Security and abuse controls

- Keep R2 private and use least-privilege Worker access.
- Enforce a hard maximum attachment size in the Server, independent of the inline threshold.
- Restrict accepted MIME types to the product's supported data and image formats.
- Sanitize the original filename before storing it as metadata.
- Generate R2 keys only from server-generated UUIDs.
- Do not put email addresses or local paths in R2 keys, issue URLs, or logs.
- Treat file IDs as opaque references, not authorization tokens.
- Authorize every feedback attachment read.
- Do not render uploaded file content as HTML in an operator UI.
- Escape or fence user content before including a preview in a GitHub Issue.

## Verification plan

### Storage tests

- A file below the threshold is stored inline and never calls R2.
- A file at the threshold uses R2.
- A file above the threshold is stored in R2 and has matching metadata.
- SHA-256 and byte size match the stored bytes.
- Reading an inline file and an R2 file returns the same bytes.
- Missing R2 objects return `file_not_found`.
- Expired files cannot be read.
- Repeated deletion succeeds without changing the result.

### Service tests

- Share creation stores a file reference rather than `resource_payload`.
- Public share reading parses both inline and R2 resources identically.
- Feedback can create multiple attachment references.
- Feedback attachment reads require authorization.
- A failed R2 upload leaves no ready file record.
- A failed metadata finalization is recoverable by cleanup.

### Worker tests

- Scheduled cleanup processes only an explicit batch size.
- Repeated scheduled invocations do not double-delete or corrupt state.
- Stale `pending` and `deleting` records are recovered after their grace period.
- R2 deletion failures increment attempts and remain retryable.
- The Worker fetch path and scheduled path use the same service construction.

### Verification commands

Run the smallest relevant checks after implementation:

```text
pnpm --dir apps/server test
pnpm --dir apps/server check
pnpm --dir apps/server worker:deploy:dry-run
pnpm check:docs
```

Use a remote R2 binding for integration verification when local emulation cannot exercise the required R2 behavior. Do not print service keys, signed URLs, user emails, or uploaded file contents in test output.

## Implementation order

1. Add the `file_objects`, `feedback_submissions`, and `feedback_attachments` migration with RLS.
2. Add the R2 binding and split Worker bindings from string configuration.
3. Implement `FileStorageService` and its Supabase metadata repository.
4. Implement inline and R2 read/write/delete tests.
5. Convert `share_links` to reference `file_id` and update `ShareService`.
6. Add the first-party feedback route and migrate Web away from the external direct submission path.
7. Add attachment authorization and GitHub Issue generation.
8. Add the scheduled cleanup handler and the conservative R2 lifecycle safety net.
9. Remove the old `resource_payload` path and obsolete external feedback client contract.
10. Run server checks, Worker dry-run, storage integration tests, and docs checks.

## Completion criteria

The implementation is complete when:

- Share and feedback both use `FileStorageService` for every file read and write.
- The database contains only metadata for R2-backed files.
- No public endpoint exposes R2 credentials or unrestricted object keys.
- Expiration is enforced on reads, not only by background cleanup.
- Cloudflare scheduled cleanup removes expired inline and R2-backed files.
- Repeated cleanup and partial failures are safe to retry.
- R2 lifecycle is configured as a fallback, not the primary retention mechanism.
- The old share payload and external feedback storage paths are deleted.
