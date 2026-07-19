import { z } from 'zod';

export const MAX_SHARE_PAYLOAD_BYTES = 100_000;
export const MAX_SHARE_PATH_DEPTH = 64;
export const MAX_SHARE_WORKSPACE_PANES = 32;

export const supportedEditorLanguageIdSchema = z.enum(['json', 'yaml', 'toml', 'javascript', 'python']);
export type SupportedEditorLanguageId = z.infer<typeof supportedEditorLanguageIdSchema>;

const pathSegmentSchema = z.discriminatedUnion('type', [
  z.object({ type: z.literal('key'), key: z.string().min(1).max(1_024) }).strict(),
  z.object({ type: z.literal('index'), index: z.number().int().nonnegative() }).strict(),
]);
export type SharePathSegment = z.infer<typeof pathSegmentSchema>;

const pathSchema = z.array(pathSegmentSchema).max(MAX_SHARE_PATH_DEPTH);
const textDocumentSchema = z.object({ text: z.string(), languageId: supportedEditorLanguageIdSchema }).strict();
const viewportAnchorSchema = z.object({ topLine: z.number().int().positive(), scrollLeft: z.number().finite().nonnegative() }).strict();
const editorSelectionSchema = z.object({
  startLine: z.number().int().positive(), startColumn: z.number().int().positive(),
  endLine: z.number().int().positive(), endColumn: z.number().int().positive(),
}).strict().refine((selection) => selection.endLine > selection.startLine || (selection.endLine === selection.startLine && selection.endColumn >= selection.startColumn), 'selection end must not precede its start');

const interactionSchema = z.object({
  treePath: pathSchema,
  focus: z.discriminatedUnion('type', [
    z.object({ type: z.literal('editor'), selection: editorSelectionSchema }).strict(),
    z.object({ type: z.literal('graph'), path: pathSchema, target: z.enum(['key', 'value', 'node']) }).strict(),
  ]).nullable(),
  subgraphWorkspace: z.object({ panePaths: z.array(pathSchema).max(MAX_SHARE_WORKSPACE_PANES) }).strict(),
}).strict();

const compareActionSchema = z.discriminatedUnion('type', [
  z.object({ type: z.literal('compare') }).strict(),
  z.object({ type: z.literal('viewport_changed'), payload: z.object({ left: viewportAnchorSchema, right: viewportAnchorSchema }).strict() }).strict(),
]);

export const shareResourceSchema = z.discriminatedUnion('type', [
  z.object({ type: z.literal('compare'), payload: z.object({ schemaVersion: z.literal(1), left: textDocumentSchema, right: textDocumentSchema, actions: z.array(compareActionSchema).min(1), interaction: interactionSchema }).strict() }).strict(),
  z.object({ type: z.literal('text_snapshot'), payload: z.object({ schemaVersion: z.literal(1), left: textDocumentSchema, right: textDocumentSchema.nullable(), layout: z.object({ viewMode: z.enum(['graph', 'text']), activePane: z.enum(['left', 'right']) }).strict(), interaction: interactionSchema }).strict() }).strict(),
]).superRefine((resource, context) => {
  if (resource.type === 'compare' && !resource.payload.actions.some((action) => action.type === 'compare')) context.addIssue({ code: z.ZodIssueCode.custom, path: ['payload', 'actions'], message: 'compare resources must include a compare action' });
  if (serializedPayloadBytes(resource.payload) > MAX_SHARE_PAYLOAD_BYTES) context.addIssue({ code: z.ZodIssueCode.custom, path: ['payload'], message: `resource.payload must be <= ${MAX_SHARE_PAYLOAD_BYTES} bytes when serialized to JSON` });
});

export type TextDocument = z.infer<typeof textDocumentSchema>;
export type ViewportAnchor = z.infer<typeof viewportAnchorSchema>;
export type EditorSelection = z.infer<typeof editorSelectionSchema>;
export type ShareInteraction = z.infer<typeof interactionSchema>;
export type CompareAction = z.infer<typeof compareActionSchema>;
export type ShareResource = z.infer<typeof shareResourceSchema>;
export type ShareResourceType = ShareResource['type'];

export function serializedPayloadBytes(payload: unknown): number {
  return new TextEncoder().encode(JSON.stringify(payload)).byteLength;
}

export function parseShareResource(value: unknown): ShareResource | null {
  const parsed = shareResourceSchema.safeParse(value);
  return parsed.success ? parsed.data : null;
}
