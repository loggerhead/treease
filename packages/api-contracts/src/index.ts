import { z } from 'zod';
import { shareResourceSchema } from '@treease/share-protocol';

export const subscriptionTierSchema = z.enum(['free', 'pro']);
export type SubscriptionTier = z.infer<typeof subscriptionTierSchema>;

export const billingCadenceSchema = z.enum(['monthly', 'yearly']);
export type BillingCadence = z.infer<typeof billingCadenceSchema>;

export const billingPriceIdSchema = billingCadenceSchema;
export type BillingPriceId = z.infer<typeof billingPriceIdSchema>;

export const usageCapabilitySchema = z.enum(['bidirectional_edit', 'large_file_processing', 'ai_suggestion']);
export type UsageCapability = z.infer<typeof usageCapabilitySchema>;
export type RecordedUsageCapability = Exclude<UsageCapability, 'ai_suggestion'>;

export const entitlementLimitSchema = z.discriminatedUnion('kind', [
  z.object({ kind: z.literal('limited'), limit: z.number().int().nonnegative() }).strict(),
  z.object({ kind: z.literal('unlimited') }).strict(),
]);
export type EntitlementLimit = z.infer<typeof entitlementLimitSchema>;

const usageLimitsSchema = z.object({
  bidirectionalEditDocumentsMonthly: entitlementLimitSchema,
  largeFileProcessingRunsMonthly: entitlementLimitSchema,
  aiProcessingMonthly: entitlementLimitSchema,
  shareMaxAgeDays: z.number().int().positive(),
}).strict();

export const usageSummarySchema = z.object({
  tier: subscriptionTierSchema,
  periodKey: z.string().regex(/^\d{4}-\d{2}$/),
  limits: usageLimitsSchema,
  usage: z.record(z.string(), z.number().nonnegative()),
}).strict();
export type UsageSummary = z.infer<typeof usageSummarySchema>;

const subscriptionStatusSchema = z.enum(['active', 'inactive', 'past_due', 'canceled']);

export type CurrentSubscription = {
  id: string;
  userId: string;
  tier: SubscriptionTier;
  billingCadence: BillingCadence | null;
  status: 'active' | 'inactive' | 'past_due' | 'canceled';
  currentPeriodEnd: string | null;
  createdAt: string;
  updatedAt: string;
  billingManagementAvailable: boolean;
};

export const currentSubscriptionSchema = z.object({
  id: z.string().min(1),
  userId: z.string().min(1),
  tier: subscriptionTierSchema,
  billingCadence: z.union([billingCadenceSchema, z.null()]),
  status: subscriptionStatusSchema,
  currentPeriodEnd: z.union([z.string().datetime({ offset: true }), z.null()]),
  createdAt: z.string().datetime({ offset: true }),
  updatedAt: z.string().datetime({ offset: true }),
  billingManagementAvailable: z.boolean(),
}).strict().transform((value): CurrentSubscription => ({
  ...value,
  billingCadence: value.billingCadence ?? null,
  currentPeriodEnd: value.currentPeriodEnd ?? null,
}));

export const accountSummarySchema = z.object({
  user: z.object({
    id: z.string().min(1),
    email: z.email().nullable(),
    avatarUrl: z.string().url().nullable(),
  }).strict(),
  subscription: currentSubscriptionSchema,
  usage: usageSummarySchema,
}).strict();
export type AccountSummary = z.infer<typeof accountSummarySchema>;

export const billingPlanDefinitionSchema = z.object({
  priceId: billingPriceIdSchema,
  tier: z.literal('pro'),
  cadence: billingCadenceSchema,
}).strict();
export type BillingPlanDefinition = z.infer<typeof billingPlanDefinitionSchema>;

export const billingPlanPriceSchema = z.object({
  priceId: billingPriceIdSchema,
  amount: z.number().nonnegative(),
  currency: z.string().min(1),
  interval: z.enum(['day', 'week', 'month', 'year']),
  intervalCount: z.number().int().positive(),
}).strict();
export type BillingPlanPrice = z.infer<typeof billingPlanPriceSchema>;

export const billingCheckoutLinkSchema = z.object({
  priceId: billingPriceIdSchema,
  url: z.url(),
}).strict();
export type BillingCheckoutLink = z.infer<typeof billingCheckoutLinkSchema>;

export const billingPortalLinkSchema = z.object({ url: z.url() }).strict();
export type BillingPortalLink = z.infer<typeof billingPortalLinkSchema>;

export const billingPricingPrewarmResponseSchema = z.object({
  plans: z.array(billingPlanPriceSchema),
  checkouts: z.array(billingCheckoutLinkSchema).nullable(),
  subscription: z.union([currentSubscriptionSchema, z.null()]),
}).strict();
export type BillingPricingPrewarm = z.infer<typeof billingPricingPrewarmResponseSchema>;

export const shareLinkSchema = z.object({
  id: z.string().min(1),
  shareUrl: z.url(),
  expiresAt: z.string().datetime({ offset: true }),
  createdAt: z.string().datetime({ offset: true }),
}).strict();
export type ShareLink = z.infer<typeof shareLinkSchema>;

export const publicShareResponseSchema = z.object({
  resourceType: z.enum(['compare', 'text_snapshot']),
  resourcePayload: z.unknown(),
}).strict();
export type PublicShareResponse = z.infer<typeof publicShareResponseSchema>;

export const structLanguageIds = [
  'typescript',
  'go',
  'rust',
  'python',
  'java',
  'kotlin',
  'csharp',
  'swift',
  'dart',
  'ruby',
  'php',
] as const;
export const structLanguageSchema = z.enum(structLanguageIds);
export type StructLanguage = z.infer<typeof structLanguageSchema>;

export const errorResponseSchema = z.object({
  error: z.string().optional(),
  message: z.string().optional(),
  details: z.unknown().optional(),
  requestId: z.string().optional(),
}).strict();

export const createCheckoutLinkSchema = z.object({
  priceId: billingPriceIdSchema,
  successUrl: z.url().optional(),
}).strict();
export const createPortalLinkSchema = z.object({}).strict();
export const changePlanSchema = z.object({ priceId: billingPriceIdSchema }).strict();
export const pricingPrewarmSchema = z.object({ successUrl: z.url().optional() }).strict();

export const suggestYqSchema = z.object({
  instruction: z.string().min(1).max(2_000),
  editorTextSnapshot: z.string().min(1).max(100_000).optional(),
  treePathSet: z.array(z.string().min(1).max(1_024)).max(200).optional(),
}).refine((value) => value.editorTextSnapshot || (value.treePathSet?.length ?? 0) > 0, {
  message: 'editorTextSnapshot or treePathSet is required',
  path: ['editorTextSnapshot'],
});

export const suggestYqResponseSchema = z.object({ expression: z.string().min(1) }).strict();

export const structGenerationSchema = z.object({
  sourceJson: z.string().min(2).max(1_000_000),
  targetLanguage: structLanguageSchema,
  rootName: z.string().regex(/^[A-Za-z_][A-Za-z0-9_]*$/).max(80).optional(),
}).strict();
export const structGenerationResponseSchema = z.object({
  language: structLanguageSchema,
  code: z.string(),
}).strict();

export const createShareSchema = z.object({
  resource: shareResourceSchema,
  expiresInDays: z.number().int().positive().optional(),
}).strict();

export const recordedUsageSchema = z.object({
  capability: z.enum(['bidirectional_edit', 'large_file_processing']),
  idempotencyKey: z.string().min(1).max(256),
  clientId: z.string().min(1).max(512),
  metadata: z.record(z.string(), z.unknown()).default({}),
}).strict();
export const clientQuerySchema = z.object({ clientId: z.string().min(1).max(512).optional() });
export const claimSchema = z.object({ clientId: z.string().min(1).max(512) }).strict();
export const claimUsageResponseSchema = z.object({ claimed: z.number().int().nonnegative() }).strict();

export const feedbackSubmissionSchema = z.object({
  category: z.enum(['bug', 'feature', 'question']),
  description: z.string().min(1).max(10_000),
  email: z.email().nullable().optional(),
  attachments: z.array(z.object({
    role: z.enum(['data_file', 'screenshot', 'console_logs']),
    fileName: z.string().min(1).max(180),
    contentType: z.string().min(1).max(100),
    bytes: z.instanceof(Uint8Array),
  }).strict()).max(5),
}).strict();
export const feedbackResponseSchema = z.object({
  id: z.string().min(1),
  issueUrl: z.url().nullable(),
}).strict();

export type SuggestYqInput = z.infer<typeof suggestYqSchema>;
export type StructGenerationInput = z.infer<typeof structGenerationSchema>;
export type CreateShareInput = z.infer<typeof createShareSchema>;
export type RecordedUsageInput = z.infer<typeof recordedUsageSchema>;
export type FeedbackSubmission = z.infer<typeof feedbackSubmissionSchema>;
