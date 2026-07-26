import type { SupportedLanguageId } from './monaco/language-support'

export type CommandId =
  | 'format' | 'minify' | 'compact' | 'sort' | 'show-yq-input' | 'generate-struct' | 'toggle-nest' | 'toggle-auto-format' | 'escape' | 'unescape'
  | 'workspace:new' | 'workspace:open' | 'workspace:save' | 'workspace:save-as' | 'workspace:close-tab'

export type CommandLangs = '*' | SupportedLanguageId

export type CommandItem = {
  id: CommandId
  label: string
  keywords: string[]
  description?: string
  type?: 'action' | 'toggle'
  langs: CommandLangs[]
}

export const commandItems: CommandItem[] = [
  { id: 'format', label: 'Format', keywords: ['pretty', 'beautify'], type: 'action', langs: ['*'] },
  { id: 'minify', label: 'Minify', keywords: ['compress', 'compact'], type: 'action', langs: ['*'] },
  {
    id: 'compact',
    label: 'Compact',
    keywords: ['zero values', 'remove empty', 'clean', 'prune'],
    description: 'Recursively remove zero-valued object entries and array elements, including null, false, zero, empty strings, empty arrays, and empty objects.',
    type: 'action',
    langs: ['*'],
  },
  { id: 'sort', label: 'Sort', keywords: ['order', 'stable'], type: 'action', langs: ['*'] },
  {
    id: 'show-yq-input',
    label: 'Show yq input box',
    keywords: ['yq', 'expression', 'query', 'transform', 'preview'],
    description: 'Open the yq expression box below the editor. Enter a query to transform or inspect the current document and preview the result without replacing the source text.',
    type: 'action',
    langs: ['*'],
  },
  {
    id: 'generate-struct',
    label: 'Generate structure definition',
    keywords: ['json', 'type', 'interface', 'struct', 'class', 'codegen'],
    description: 'Generate a typed structure definition from the active document. Non-JSON documents are converted to JSON first.',
    type: 'action',
    langs: ['*'],
  },
  {
    id: 'toggle-nest',
    label: 'Enable nest parse',
    keywords: ['nested', 'json', 'expand', 'nest'],
    description: 'Parse nested JSON strings into their structured values so embedded objects and arrays can be viewed and edited as nested data.',
    type: 'toggle',
    langs: ['json'],
  },
  {
    id: 'toggle-auto-format',
    label: 'Enable auto format',
    keywords: ['smart', 'auto', 'format', 'beautify'],
    description: 'Automatically apply the editor’s formatting rules after supported document changes, keeping indentation and layout synchronized with the active format settings.',
    type: 'toggle',
    langs: ['*'],
  },
  { id: 'escape', label: 'Escape', keywords: ['escape', 'encode', 'to_json', 'stringify'], type: 'action', langs: ['json'] },
  { id: 'unescape', label: 'Unescape', keywords: ['unescape', 'decode', 'from_json', 'parse'], type: 'action', langs: ['json'] },
]
