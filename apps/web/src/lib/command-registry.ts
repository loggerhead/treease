import type { SupportedLanguageId } from './monaco/language-support'

export type CommandId = 'format' | 'minify' | 'sort' | 'show-yq-input' | 'toggle-nest' | 'toggle-auto-format' | 'escape' | 'unescape'

export type CommandLangs = '*' | SupportedLanguageId

export type CommandItem = {
  id: CommandId
  label: string
  keywords: string[]
  type?: 'action' | 'toggle'
  langs: CommandLangs[]
}

export const commandItems: CommandItem[] = [
  { id: 'format', label: 'Format', keywords: ['pretty', 'beautify'], type: 'action', langs: ['*'] },
  { id: 'minify', label: 'Minify', keywords: ['compress', 'compact'], type: 'action', langs: ['*'] },
  { id: 'sort', label: 'Sort', keywords: ['order', 'stable'], type: 'action', langs: ['*'] },
  { id: 'show-yq-input', label: 'Show yq input box', keywords: ['yq', 'expression', 'query', 'transform', 'preview'], type: 'action', langs: ['*'] },
  { id: 'toggle-nest', label: 'Enable nest parse', keywords: ['nested', 'json', 'expand', 'nest'], type: 'toggle', langs: ['json'] },
  { id: 'toggle-auto-format', label: 'Enable auto format', keywords: ['smart', 'auto', 'format', 'beautify'], type: 'toggle', langs: ['*'] },
  { id: 'escape', label: 'Escape', keywords: ['escape', 'encode', 'to_json', 'stringify'], type: 'action', langs: ['json'] },
  { id: 'unescape', label: 'Unescape', keywords: ['unescape', 'decode', 'from_json', 'parse'], type: 'action', langs: ['json'] },
]
