import { guessLanguage } from '../guess/guess-language'
import {
  findSupportedLanguageByExtension,
  type SupportedLanguageId,
} from '../monaco/language-support'

const IMPORT_SOURCE_SAMPLE_BYTES = 1024

export function findFormatByExtension(fileName: string): SupportedLanguageId | null {
  const dotIndex = fileName.lastIndexOf('.')
  if (dotIndex <= 0 || dotIndex === fileName.length - 1) return null
  const ext = fileName.slice(dotIndex).toLowerCase()
  if (ext === '.txt') return null
  return findSupportedLanguageByExtension(ext)
}

export async function readImportSourceSample(file: File, byteLimit = IMPORT_SOURCE_SAMPLE_BYTES): Promise<string> {
  return file.slice(0, byteLimit).text();
}

export async function resolveImportSourceFormat(
  fileName: string,
  text: string,
  fallbackFormat: SupportedLanguageId,
): Promise<SupportedLanguageId> {
  const byExtension = findFormatByExtension(fileName)
  if (byExtension) return byExtension
  const guessed = await guessLanguage(text)
  return guessed ?? fallbackFormat
}
