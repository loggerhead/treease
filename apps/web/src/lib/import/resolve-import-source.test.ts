import { beforeEach, describe, expect, it, vi } from 'vitest'

vi.mock('../guess/guess-language', () => ({
  guessLanguage: vi.fn()
}))

import { guessLanguage } from '../guess/guess-language'
import { findFormatByExtension, readImportSourceSample, resolveImportSourceFormat } from './resolve-import-source'

describe('resolve-import-source', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('detects format by file extension first', async () => {
    const result = await resolveImportSourceFormat('data.yaml', 'a: 1', 'json')

    expect(result).toBe('yaml')
    expect(guessLanguage).not.toHaveBeenCalled()
  })

  it('falls back to lightweight language guess when extension is unknown', async () => {
    vi.mocked(guessLanguage).mockResolvedValueOnce('toml')

    const result = await resolveImportSourceFormat('README.txt', 'a = 1', 'json')

    expect(result).toBe('toml')
    expect(guessLanguage).toHaveBeenCalledWith('a = 1')
  })

  it('falls back to provided format when guess returns null', async () => {
    vi.mocked(guessLanguage).mockResolvedValueOnce(null)

    const result = await resolveImportSourceFormat('README.txt', 'plain text', 'json')

    expect(result).toBe('json')
  })

  it('reads only the first 1KB from a streamed file sample', async () => {
    const file = new File(['a'.repeat(1500)], 'README.txt', { type: 'text/plain' })

    const sample = await readImportSourceSample(file)

    expect(sample).toBe('a'.repeat(1024))
  })

  it('returns null when file extension is not supported', () => {
    expect(findFormatByExtension('README.txt')).toBeNull()
    expect(findFormatByExtension('README')).toBeNull()
  })
})
