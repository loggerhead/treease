import type { PageServerLoad } from './$types';
import { EDITOR_SPLIT_RATIO_COOKIE, readEditorSplitRatioCookie } from '../../lib/settings/editor-layout-cookie';

export const load: PageServerLoad = ({ cookies }) => ({
  editorSplitRatio: readEditorSplitRatioCookie(cookies.get(EDITOR_SPLIT_RATIO_COOKIE)),
});
