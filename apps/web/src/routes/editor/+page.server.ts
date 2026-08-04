import type { PageServerLoad } from './$types';
import { EDITOR_SPLIT_RATIO_COOKIE, readEditorSplitRatioCookie, readSidebarExpandedCookie, SIDEBAR_EXPANDED_COOKIE } from '../../lib/settings/editor-layout-cookie';

export const load: PageServerLoad = ({ cookies }) => ({
  editorSplitRatio: readEditorSplitRatioCookie(cookies.get(EDITOR_SPLIT_RATIO_COOKIE)),
  sidebarExpanded: readSidebarExpandedCookie(cookies.get(SIDEBAR_EXPANDED_COOKIE)),
});
