import type { MonacoApi } from './public-types'

const YQ_LANGUAGE_ID = 'treease-yq'

const yqKeywords = [
  'SELF',
  'all',
  'and',
  'any',
  'array_to_map',
  'as',
  'ascii_downcase',
  'ascii_upcase',
  'asciidowncase',
  'asciiupcase',
  'capture',
  'contains',
  'del',
  'delpaths',
  'documentIndex',
  'downcase',
  'fi',
  'fileIndex',
  'filter',
  'first',
  'flatten',
  'fromEntries',
  'from_csv',
  'from_entries',
  'from_json',
  'from_yaml',
  'group_by',
  'has',
  'is_key',
  'iskey',
  'join',
  'key',
  'keys',
  'kind',
  'length',
  'load',
  'load_str',
  'map',
  'map_values',
  'match',
  'max',
  'min',
  'not',
  'omit',
  'or',
  'parent',
  'parents',
  'path',
  'pick',
  'reduce',
  'reverse',
  'root',
  'select',
  'setpath',
  'shuffle',
  'sort',
  'sort_by',
  'sort_keys',
  'sortKeys',
  'split',
  'sub',
  'tag',
  'test',
  'to_csv',
  'to_entries',
  'to_json',
  'to_toml',
  'to_number',
  'to_string',
  'to_yaml',
  'tonumber',
  'tostring',
  'trim',
  'type',
  'unique',
  'unique_by',
  'upcase',
  'with',
  'withEntries',
  'with_entries'
] as const

const yqConstants = ['true', 'false', 'null'] as const
const yqCodecs = ['@yaml', '@json', '@toml', '@csv', '@base64', '@yamld', '@jsond', '@tomld', '@csvd', '@base64d'] as const
const yqCompletionLabels = [...yqKeywords, ...yqConstants, ...yqCodecs] as const

const keywordPattern = yqKeywords.slice().sort((a, b) => b.length - a.length).map(escapeRegex).join('|')
const constantPattern = yqConstants.map(escapeRegex).join('|')
const codecNamePattern = yqCodecs.map((value) => escapeRegex(value.slice(1))).join('|')

function escapeRegex(value: string) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
}

function normalizeCompletionLabel(value: string) {
  return value.toLowerCase().replace(/^@/, '')
}

export function hasYqCompletionMatches(value: string) {
  const normalizedValue = normalizeCompletionLabel(value.trim())
  if (!normalizedValue) return yqCompletionLabels.length > 0
  return yqCompletionLabels.some((label) => normalizeCompletionLabel(label).startsWith(normalizedValue))
}

export { YQ_LANGUAGE_ID, yqKeywords }

const yqSupportStateByMonaco = new WeakMap<object, {
  languageRegistered: boolean
  completionRegistered: boolean
  monarchRegistered: boolean
}>()

export function createYqLanguageSupportRegistrar(options: {
  monaco: MonacoApi
}) {
  const { monaco } = options
  const registryKey = monaco as object

  return function ensureYqLanguageSupport() {
    const state = yqSupportStateByMonaco.get(registryKey) ?? {
      languageRegistered: false,
      completionRegistered: false,
      monarchRegistered: false
    }

    if (!yqSupportStateByMonaco.has(registryKey)) {
      yqSupportStateByMonaco.set(registryKey, state)
    }

    if (!state.languageRegistered) {
      monaco.languages.register({ id: YQ_LANGUAGE_ID })
      monaco.languages.setLanguageConfiguration(YQ_LANGUAGE_ID, {
        autoClosingPairs: [
          { open: '(', close: ')' },
          { open: '[', close: ']' },
          { open: '{', close: '}' },
          { open: '"', close: '"' },
          { open: "'", close: "'" }
        ],
        surroundingPairs: [
          { open: '(', close: ')' },
          { open: '[', close: ']' },
          { open: '{', close: '}' },
          { open: '"', close: '"' },
          { open: "'", close: "'" }
        ]
      })
      state.languageRegistered = true
    }

    if (!state.monarchRegistered) {
      monaco.languages.setMonarchTokensProvider(YQ_LANGUAGE_ID, {
        tokenizer: {
          root: [
            [/\s+/, 'white'],
            [new RegExp(`@(?:${codecNamePattern})\\b`), 'keyword'],
            [new RegExp(`\\b(${keywordPattern})\\b`), 'keyword'],
            [new RegExp(`\\b(${constantPattern})\\b`), 'constant'],
            [/\$[A-Za-z_][\w-]*/, 'variable'],
            [/-?\d+(?:\.\d+)?/, 'number'],
            [/"([^"\\]|\\.)*"?/, 'string'],
            [/'([^'\\]|\\.)*'?/, 'string'],
            [/\|=|\+=|-=|\*=|\/\/|==|!=|<=|>=|[+\-*/%<>|=]/, 'operator'],
            [/\.{1,2}|\(|\)|\[|\]|\{|\}|,|:/, 'delimiter'],
            [/[A-Za-z_][\w-]*/, 'identifier']
          ]
        }
      })
      state.monarchRegistered = true
    }

    if (!state.completionRegistered) {
      const suggestions = [
        ...yqKeywords.map((label) => ({
          label,
          kind: monaco.languages.CompletionItemKind.Function,
          insertText: label,
          detail: 'yq operator'
        })),
        ...yqConstants.map((label) => ({
          label,
          kind: monaco.languages.CompletionItemKind.Keyword,
          insertText: label,
          detail: 'literal'
        })),
        ...yqCodecs.map((label) => ({
          label,
          kind: monaco.languages.CompletionItemKind.Keyword,
          insertText: label,
          detail: 'codec'
        }))
      ]

      monaco.languages.registerCompletionItemProvider(YQ_LANGUAGE_ID, {
        triggerCharacters: ['.', '|', '(', '@'],
        provideCompletionItems(model, position) {
          const word = model.getWordUntilPosition(position)
          const range = {
            startLineNumber: position.lineNumber,
            endLineNumber: position.lineNumber,
            startColumn: word.startColumn,
            endColumn: word.endColumn
          }
          return {
            suggestions: suggestions.map((item) => ({ ...item, range }))
          }
        }
      })
      state.completionRegistered = true
    }
  }
}
