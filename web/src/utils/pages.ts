/**
 * Page sizing shared by the UI counters and the generation request.
 *
 * One page = the characters that fit on a page of the exported .docx
 * (Calibri 12pt, 1.5 line spacing, 1"/1.25" margins — see wordExport.ts).
 * Must match `chars_per_page` in backend_py/src/edu_backend/config.py.
 */
export const CHARS_PER_PAGE = 2000;

/** Hard ceiling above the requested page count ("+5 at most"). */
export const PAGE_CEILING_MARGIN = 5;

export function estimatePages(text: string): number {
  return Math.max(1, Math.ceil(text.length / CHARS_PER_PAGE));
}
