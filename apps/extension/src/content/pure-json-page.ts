import { extractCandidate, type CandidateExtraction } from './extract-candidate';

/**
 * Mirrors the conservative raw-document branch used by JSON formatting tools:
 * only a browser-rendered, single-`pre` document is considered, then strict
 * JSON.parse confirms that the entire document is JSON. Regular pages are not
 * scanned.
 */
export function extractPureJsonPage(documentValue: Document = document): CandidateExtraction {
  if (!/^(application\/json|text\/plain)(?:;|$)/i.test(documentValue.contentType)) return { status: 'none' };
  const body = documentValue.body;
  if (!body || body.children.length !== 1) return { status: 'none' };
  const pre = body.firstElementChild;
  if (!(pre instanceof HTMLPreElement)) return { status: 'none' };
  const extracted = extractCandidate(pre);
  if (extracted.status !== 'candidate') return extracted;
  try {
    JSON.parse(extracted.text);
    return extracted;
  } catch {
    return { status: 'none' };
  }
}
