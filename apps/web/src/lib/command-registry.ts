import type { SupportedLanguageId } from './monaco/language-support'

export type CommandId =
  | 'format' | 'minify' | 'sort' | 'show-yq-input' | 'toggle-nest' | 'toggle-auto-format' | 'escape' | 'unescape'
  | 'workspace:new' | 'workspace:open' | 'workspace:save' | 'workspace:save-as' | 'workspace:close-tab'

export type CommandLangs = '*' | SupportedLanguageId

export type CommandItem = {
  id: CommandId
  label: string
  keywords: string[]
  type?: 'action' | 'toggle'
  langs: CommandLangs[]
}

export const commandItems: CommandItem[] = [
  { id: 'workspace:new', label: 'New document', keywords: ['file', 'tab', 'create'], type: 'action', langs: ['*'] },
  { id: 'workspace:open', label: 'Open document', keywords: ['file', 'import'], type: 'action', langs: ['*'] },
  { id: 'workspace:save', label: 'Save document', keywords: ['file', 'write'], type: 'action', langs: ['*'] },
  { id: 'workspace:save-as', label: 'Save document as', keywords: ['file', 'write', 'copy'], type: 'action', langs: ['*'] },
  { id: 'workspace:close-tab', label: 'Close tab', keywords: ['file', 'document'], type: 'action', langs: ['*'] },
  { id: 'format', label: 'Format', keywords: ['pretty', 'beautify'], type: 'action', langs: ['*'] },
  { id: 'minify', label: 'Minify', keywords: ['compress', 'compact'], type: 'action', langs: ['*'] },
  { id: 'sort', label: 'Sort', keywords: ['order', 'stable'], type: 'action', langs: ['*'] },
  { id: 'show-yq-input', label: 'Show yq input box', keywords: ['yq', 'expression', 'query', 'transform', 'preview'], type: 'action', langs: ['*'] },
  { id: 'toggle-nest', label: 'Enable nest parse', keywords: ['nested', 'json', 'expand', 'nest'], type: 'toggle', langs: ['json'] },
  { id: 'toggle-auto-format', label: 'Enable auto format', keywords: ['smart', 'auto', 'format', 'beautify'], type: 'toggle', langs: ['*'] },
  { id: 'escape', label: 'Escape', keywords: ['escape', 'encode', 'to_json', 'stringify'], type: 'action', langs: ['json'] },
  { id: 'unescape', label: 'Unescape', keywords: ['unescape', 'decode', 'from_json', 'parse'], type: 'action', langs: ['json'] },
]
