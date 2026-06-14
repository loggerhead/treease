import type { SupportedEditorLanguageId } from '../monaco/language-support';

type DiagnosticsError = {
  startLineNumber: number;
  startColumn: number;
  endLineNumber: number;
  endColumn: number;
  kind: number;
};

type DiagnosticsProvider = (language: SupportedEditorLanguageId, text: string) => Promise<DiagnosticsError[]>;

type FeatureStats = {
  firstChar: string;
  hasBrace: boolean;
  hasBracket: boolean;
  hasColon: boolean;
  hasEqual: boolean;
  hasSyntaxEqual: boolean;
  hasDoubleQuote: boolean;
  hasSingleQuote: boolean;
  hasHashComment: boolean;
  hasSlashComment: boolean;
  hasIndent: boolean;
  hasYamlList: boolean;
  hasTomlSection: boolean;
  hasTomlBareSection: boolean;
  hasTomlKeyValue: boolean;
  hasTomlCommentOnly: boolean;
  hasJsonScalar: boolean;
  hasJsonContainer: boolean;
  hasJsonSimpleArray: boolean;
  hasYamlDocStart: boolean;
  hasYamlDocumentMarker: boolean;
  hasYamlDirective: boolean;
  hasYamlTag: boolean;
  hasYamlAnchorAlias: boolean;
  hasYamlExplicitKey: boolean;
  hasYamlBlockScalar: boolean;
  hasYamlBareMapping: boolean;
  hasYamlQuotedMappingKey: boolean;
  hasYamlListFlowMapping: boolean;
  hasYamlMultilineQuotedScalar: boolean;
  hasYamlSingleQuotedScalar: boolean;
  hasYamlFlowCollection: boolean;
  hasJsonQuotedKey: boolean;
  hasSingleQuotedKey: boolean;
  hasUnquotedKey: boolean;
  hasTrailingComma: boolean;
  hasPythonLiteral: boolean;
  hasColonNewline: boolean;
};

type GuessResult = {
  language: SupportedEditorLanguageId | null;
  score: number;
  reason: string;
};

const MAX_SAMPLE_LENGTH = 1024;
const MIN_SAMPLE_LENGTH = 8;

const FEATURE_HIT_WEIGHT = 15;
const AMBIGUITY_PENALTY_WEIGHT = 30;
const DIAGNOSTIC_SUCCESS_BONUS = 1000;
const DIAGNOSTIC_ERROR_OFFSET_WEIGHT = 0.2;
const LOOSE_PARSER_PENALTY = 50;
const MIN_DIAGNOSTIC_SCORE = 10;
const MIN_DIAGNOSTIC_SCORE_GAP = 50;

const ambiguityPenalty: Record<SupportedEditorLanguageId, number> = {
  json: 0.2,
  yaml: 1.2,
  toml: 0.5,
  python: 1.5,
  javascript: 2,
};

const looseParserPenaltyLanguages = new Set<SupportedEditorLanguageId>(['javascript', 'python']);

function truncateInput(text: string): string {
  if (text.length <= MAX_SAMPLE_LENGTH) return text;
  return text.slice(0, MAX_SAMPLE_LENGTH);
}

function findFirstNonWhitespace(text: string): string {
  for (let i = 0; i < text.length; i += 1) {
    const ch = text[i];
    if (!/\s/.test(ch)) return ch;
  }
  return '';
}

function stripQuotedContent(text: string): string {
  let result = '';
  let quote: '"' | "'" | null = null;
  let escaped = false;
  for (const ch of text) {
    if (quote) {
      result += ch === '\n' || ch === '\r' ? ch : ' ';
      if (escaped) {
        escaped = false;
      } else if (ch === '\\') {
        escaped = true;
      } else if (ch === quote) {
        quote = null;
      }
      continue;
    }
    if (ch === '"' || ch === "'") {
      quote = ch;
      result += ' ';
      continue;
    }
    result += ch;
  }
  return result;
}

function trimBomAndWhitespace(text: string): string {
  return text.replace(/^\uFEFF/, '').trim();
}

function scanFeatures(text: string): FeatureStats {
  const syntaxText = stripQuotedContent(text);
  const trimmed = trimBomAndWhitespace(text);
  const hasBrace = text.includes('{') || text.includes('}');
  const hasBracket = text.includes('[') || text.includes(']');
  const hasColon = text.includes(':');
  const hasEqual = text.includes('=');
  const hasSyntaxEqual = syntaxText.includes('=');
  const hasDoubleQuote = text.includes('"');
  const hasSingleQuote = text.includes("'");
  const hasHashComment = /^\s*#/m.test(text);
  const hasSlashComment = text.includes('//') || text.includes('/*');
  const hasIndent = /^[ \t]+/m.test(text);
  const hasYamlList = /^\s*-\s+\S+/m.test(text);
  const hasTomlSection = /^\s*(?:\[\[[^\],]+\]\]|\[[^\],]+\])\s*(?:#.*)?$/m.test(syntaxText);
  const hasTomlBareSection = /^\s*(?:\[\[\s*[A-Za-z0-9_-][^\],]*\]\]|\[\s*[A-Za-z0-9_-][^\],]*\])\s*(?:#.*)?$/m.test(
    text,
  );
  const hasTomlKeyValue =
    /^\s*(?:"[^"\r\n]*"|'[^'\r\n]*'|[A-Za-z0-9_-]+)(?:\s*\.\s*(?:"[^"\r\n]*"|'[^'\r\n]*'|[A-Za-z0-9_-]+))*\s*=/m.test(
      text,
    );
  const hasTomlCommentOnly = trimmed.length > 0 && /^(?:\s*#.*(?:\r?\n|$))+$/.test(text);
  const hasJsonScalar = /^(?:-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?|true|false|null|"(?:\\.|[^"\\\r\n])*")$/.test(
    trimmed,
  );
  const hasJsonContainer = /^(?:\{|\[)/.test(trimmed);
  const hasJsonSimpleArray =
    /^\[\s*(?:(?:"(?:\\.|[^"\\])*"|-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?|true|false|null|\{\}|\[\])\s*,\s*)*(?:"(?:\\.|[^"\\])*"|-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?|true|false|null|\{\}|\[\])?\s*\]$/s.test(
      trimmed,
    );
  const hasYamlDocStart = /^\s*---\s*$/m.test(text);
  const hasYamlDocumentMarker = /^\s*(?:---|\.\.\.)(?:\s|$)/m.test(text);
  const hasYamlDirective = /^\s*%[A-Z][A-Z0-9-]*(?:\s|$)/m.test(text);
  const hasYamlTag = /(^|\s)!(?:![A-Za-z0-9_-]+|[A-Za-z][A-Za-z0-9_-]*|<[^>\r\n]+>|\s+\S|$)/.test(text);
  const hasYamlAnchorAlias = /(^|\s)[&*][A-Za-z0-9_-]+/.test(text);
  const hasYamlExplicitKey = /^\s*[?:](?:\s|:|$)|^\s*\[\s*\?/m.test(text);
  const hasYamlBlockScalar = /(?:^|\n)\s*(?:---\s*)?(?:[A-Za-z0-9_-][^:\n]*:\s*)?[|>][+-]?\d?(?:\s|$)/.test(text);
  const hasYamlBareMapping = /^[^\s#"'[{][^:\n]*:\s+\S/m.test(text);
  const hasYamlQuotedMappingKey = /^\s*(?:"(?:\\.|[^"\\])*"|'(?:[^']|'')*')\s*:/m.test(text);
  const hasYamlListFlowMapping = /^\s*-\s*\{[^}\r\n]*:\S/m.test(text);
  const hasYamlMultilineQuotedScalar = /^["'][\s\S]*\n[\s\S]*["']\s*$/.test(trimmed);
  const hasYamlSingleQuotedScalar = /^'(?:[^']|'')*'\s*$/.test(trimmed);
  const hasYamlFlowCollection =
    /^(?:\{|\[)/.test(trimmed) &&
    (/[{[,]\s*[A-Za-z_][^"'\n,[\]{}]*?(?:[:,])/.test(trimmed) ||
      /[{]\s*\?/.test(trimmed) ||
      /:\s*(?:,|\})/.test(trimmed));
  const hasJsonQuotedKey = /[{,]\s*"[^"\n]+"\s*:/.test(text);
  const hasSingleQuotedKey = /[{,]\s*'[^'\n]+'\s*:/.test(text);
  const hasUnquotedKey = /[{,]\s*[$A-Z_a-z][\w$]*\s*:/.test(syntaxText);
  const hasTrailingComma = /,\s*[}\]]/.test(text);
  const hasPythonLiteral = /\b(True|False|None)\b/.test(text);
  const hasColonNewline = /:\s*(?:\r?\n|$)/.test(text);
  const firstChar = findFirstNonWhitespace(text);
  return {
    firstChar,
    hasBrace,
    hasBracket,
    hasColon,
    hasEqual,
    hasSyntaxEqual,
    hasDoubleQuote,
    hasSingleQuote,
    hasHashComment,
    hasSlashComment,
    hasIndent,
    hasYamlList,
    hasTomlSection,
    hasTomlBareSection,
    hasTomlKeyValue,
  hasTomlCommentOnly,
  hasJsonScalar,
    hasJsonContainer,
    hasJsonSimpleArray,
    hasYamlDocStart,
    hasYamlDocumentMarker,
    hasYamlDirective,
    hasYamlTag,
    hasYamlAnchorAlias,
    hasYamlExplicitKey,
    hasYamlBlockScalar,
    hasYamlBareMapping,
    hasYamlQuotedMappingKey,
    hasYamlListFlowMapping,
    hasYamlMultilineQuotedScalar,
    hasYamlSingleQuotedScalar,
    hasYamlFlowCollection,
    hasJsonQuotedKey,
    hasSingleQuotedKey,
    hasUnquotedKey,
    hasTrailingComma,
    hasPythonLiteral,
    hasColonNewline,
  };
}

function pickCandidates(stats: FeatureStats): SupportedEditorLanguageId[] {
  if (stats.hasTomlSection && stats.hasTomlKeyValue && stats.firstChar !== '<') {
    return ['toml'];
  }
  if (stats.firstChar === '[' && stats.hasJsonQuotedKey && !stats.hasYamlDocumentMarker) {
    return ['json'];
  }
  if (stats.firstChar === '{' && stats.hasPythonLiteral) {
    return ['python'];
  }
  if (stats.hasYamlDocumentMarker) {
    return ['yaml'];
  }
  if (
    stats.firstChar === '{' &&
    stats.hasDoubleQuote &&
    !stats.hasUnquotedKey &&
    !stats.hasSingleQuotedKey &&
    !stats.hasTrailingComma
  ) {
    return ['json'];
  }

  const candidates = new Set<SupportedEditorLanguageId>();
  const hasYamlSpecificSyntax =
    stats.hasYamlDocumentMarker ||
    stats.hasYamlDirective ||
    stats.hasYamlTag ||
    stats.hasYamlAnchorAlias ||
    stats.hasYamlExplicitKey ||
    stats.hasYamlBlockScalar ||
    stats.hasYamlBareMapping ||
    stats.hasYamlQuotedMappingKey ||
    stats.hasYamlListFlowMapping ||
    stats.hasYamlMultilineQuotedScalar ||
    stats.hasYamlSingleQuotedScalar ||
    stats.hasYamlFlowCollection;
  const isObjectLike =
    stats.hasBrace ||
    stats.firstChar === '{' ||
    stats.firstChar === '[' ||
    stats.hasJsonScalar ||
    stats.hasJsonContainer;
  const isMappingLike =
    stats.hasColon && (stats.hasIndent || stats.hasYamlList || stats.hasYamlDocStart || stats.hasColonNewline);


  if ((stats.hasTomlKeyValue || stats.hasTomlSection || stats.hasTomlCommentOnly) && stats.firstChar !== '<') {
    candidates.add('toml');
  }

  if (stats.hasYamlList || isMappingLike || hasYamlSpecificSyntax) {
    candidates.add('yaml');
  }

  if (isObjectLike || isMappingLike) {
    candidates.add('json');
  }

  if (stats.firstChar === '{' && (stats.hasUnquotedKey || stats.hasTrailingComma)) {
    candidates.add('javascript');
  }

  if (stats.firstChar === '{' && (stats.hasSingleQuotedKey || stats.hasPythonLiteral)) {
    candidates.add('python');
  }

  if (candidates.size === 0) {
    return ['json', 'yaml', 'toml'];
  }

  return [...candidates];
}

function scoreFeatures(lang: SupportedEditorLanguageId, stats: FeatureStats): number {
  switch (lang) {
    case 'json': {
      let score = 0;
      if (stats.hasJsonScalar) score += 4;
      if (stats.hasJsonContainer) score += 4;
      if (stats.firstChar === '[') score += 3;
      if (stats.firstChar === '{' && stats.hasJsonQuotedKey) score += 3;
      if (stats.firstChar === '{' && stats.hasColon && stats.hasDoubleQuote && !stats.hasUnquotedKey) score += 4;
      if (stats.hasTomlBareSection && !stats.hasJsonSimpleArray) score -= 8;
      if (stats.hasJsonQuotedKey) score += 2;
      if (stats.hasDoubleQuote && !stats.hasSingleQuote) score += 1;
      if (stats.hasSyntaxEqual) score -= 2;
      if (stats.hasYamlListFlowMapping) score -= 4;
      if (stats.hasUnquotedKey) score -= 2;
      if (stats.hasSingleQuotedKey) score -= 2;
      if (stats.hasPythonLiteral) score -= 2;
      if (stats.hasTrailingComma) score -= 1;
      return score;
    }
    case 'javascript': {
      let score = 0;
      if (stats.hasUnquotedKey) score += 3;
      if (stats.hasUnquotedKey && !stats.hasJsonQuotedKey && !stats.hasSingleQuotedKey) score += 3;
      if (stats.hasTrailingComma) score += 1;
      if (stats.hasSlashComment) score += 1;
      if (stats.hasJsonQuotedKey) score -= 2;
      if (stats.hasPythonLiteral) score -= 1;
      return score;
    }
    case 'python': {
      let score = 0;
      if (stats.hasPythonLiteral) score += 3;
      if (stats.hasSingleQuotedKey) score += 2;
      if (stats.hasJsonQuotedKey) score -= 2;
      if (stats.hasUnquotedKey) score -= 1;
      if (stats.hasSlashComment) score -= 2;
      return score;
    }
    case 'yaml': {
      let score = 0;
      if (stats.hasYamlDocumentMarker) score += 4;
      if (stats.hasYamlDirective) score += 5;
      if (stats.hasYamlTag) score += 5;
      if (stats.hasYamlAnchorAlias) score += 5;
      if (stats.hasYamlExplicitKey) score += 4;
      if (stats.hasYamlBlockScalar) score += 4;
      if (stats.hasYamlBareMapping) score += 7;
      if (stats.hasYamlQuotedMappingKey) score += 7;
      if (stats.hasYamlListFlowMapping) score += 6;
      if (stats.hasYamlMultilineQuotedScalar) score += 5;
      if (stats.hasYamlSingleQuotedScalar) score += 5;
      if (stats.hasYamlFlowCollection) score += 5;
      if (stats.hasYamlList) score += 6;
      if (stats.hasColonNewline) score += 2;
      if (stats.hasIndent) score += 1;
      if (stats.hasBrace && !stats.hasYamlTag && !stats.hasYamlBareMapping) score -= 1;
      if (stats.hasEqual) score -= 1;
      if (stats.firstChar === '<') score -= 3;
      return score;
    }
    case 'toml': {
      let score = 0;
      if (stats.hasTomlSection && !stats.hasJsonSimpleArray) score += 6;
      if (stats.hasTomlKeyValue) score += 6;
      if (stats.hasTomlCommentOnly) score += 4;
      if (stats.hasSyntaxEqual) score += 1;
      if (stats.firstChar === '[' && !stats.hasTomlSection) score -= 3;
      if (stats.hasBrace && !stats.hasTomlKeyValue) score -= 1;
      if (stats.hasColonNewline) score -= 1;
      if (stats.hasSingleQuotedKey) score -= 3;
      if (stats.hasPythonLiteral) score -= 3;
      return score;
    }
    default:
      return 0;
  }
}

function buildLineOffsets(text: string): number[] {
  const offsets = [0];
  for (let i = 0; i < text.length; i += 1) {
    if (text[i] === '\n') offsets.push(i + 1);
  }
  return offsets;
}

function toOffset(lineOffsets: number[], lineNumber: number, column: number): number {
  const lineIndex = Math.max(0, lineNumber - 1);
  const lineOffset = lineOffsets[lineIndex] ?? lineOffsets[lineOffsets.length - 1] ?? 0;
  return lineOffset + Math.max(0, column - 1);
}

function scoreWithDiagnostics(
  lang: SupportedEditorLanguageId,
  stats: FeatureStats,
  errors: DiagnosticsError[],
  textLength: number,
  lineOffsets: number[],
): GuessResult {
  const success = errors.length === 0;
  let errorOffset = 0;
  if (!success) {
    errorOffset = errors.reduce((max, err) => {
      const end = toOffset(lineOffsets, err.endLineNumber, err.endColumn);
      return Math.max(max, end);
    }, 0);
  } else {
    errorOffset = textLength;
  }

  const featureHits = scoreFeatures(lang, stats);
  const loosePenalty = !success && looseParserPenaltyLanguages.has(lang) ? LOOSE_PARSER_PENALTY : 0;
  const score =
    DIAGNOSTIC_SUCCESS_BONUS * (success ? 1 : 0) +
    DIAGNOSTIC_ERROR_OFFSET_WEIGHT * errorOffset +
    FEATURE_HIT_WEIGHT * featureHits -
    AMBIGUITY_PENALTY_WEIGHT * ambiguityPenalty[lang] -
    loosePenalty;

  return {
    language: lang,
    score,
    reason: success ? 'parse-success' : 'parse-error',
  };
}

export async function guessLanguage(
  input: string,
  diagnosticsProvider?: DiagnosticsProvider,
): Promise<SupportedEditorLanguageId | null> {
  const raw = truncateInput(input).replace(/^\uFEFF/, '');

  const stats = scanFeatures(raw);
  if (
    raw.trim().length < MIN_SAMPLE_LENGTH &&
    !stats.hasJsonScalar &&
    !stats.hasJsonContainer &&
    !stats.hasYamlDocumentMarker &&
    !stats.hasYamlDirective &&
    !stats.hasYamlTag &&
    !stats.hasYamlAnchorAlias &&
    !stats.hasYamlExplicitKey &&
    !stats.hasYamlBlockScalar &&
    !stats.hasYamlBareMapping &&
    !stats.hasYamlQuotedMappingKey &&
    !stats.hasYamlListFlowMapping &&
    !stats.hasYamlMultilineQuotedScalar &&
    !stats.hasYamlSingleQuotedScalar &&
    !stats.hasYamlFlowCollection &&
    !stats.hasYamlList &&
    !stats.hasTomlKeyValue &&
    !stats.hasTomlSection &&
    !stats.hasTomlCommentOnly
  ) {
    return null;
  }
  const candidates = pickCandidates(stats);
  const results: GuessResult[] = [];
  if (!diagnosticsProvider) {
    for (const lang of candidates) {
      const featureHits = scoreFeatures(lang, stats);
      const score = FEATURE_HIT_WEIGHT * featureHits - AMBIGUITY_PENALTY_WEIGHT * ambiguityPenalty[lang];
      results.push({ language: lang, score, reason: 'features-only' });
    }
  } else {
    const lineOffsets = buildLineOffsets(raw);
    const diagnosticsList = await Promise.all(
      candidates.map(async (lang) => {
        try {
          const errors = await diagnosticsProvider(lang, raw);
          return { lang, errors };
        } catch {
          return { lang, errors: [{ startLineNumber: 1, startColumn: 1, endLineNumber: 1, endColumn: 1, kind: 0 }] };
        }
      }),
    );
    for (const { lang, errors } of diagnosticsList) {
      results.push(scoreWithDiagnostics(lang, stats, errors, raw.length, lineOffsets));
    }
  }

  results.sort((a, b) => b.score - a.score);
  const best = results[0];
  const second = results[1];
  if (!best || !best.language) return null;

  if (diagnosticsProvider) {
    if (second && best.score - second.score < MIN_DIAGNOSTIC_SCORE_GAP) return null;
    if (best.score < MIN_DIAGNOSTIC_SCORE) return null;
  }
  return best.language;
}
