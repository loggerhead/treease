import {
  parseShareResource,
  type ShareInteraction,
  type EditorSelection,
  type SharePathSegment,
  type ShareResource,
  type SupportedEditorLanguageId,
  type TextDocument,
  type ViewportAnchor,
} from '@treease/share-protocol';

export { parseShareResource, type EditorSelection, type ShareInteraction, type SharePathSegment, type ShareResource, type SupportedEditorLanguageId, type TextDocument, type ViewportAnchor };
export type ShareCompareKind = 'none' | 'equal' | 'different';

export function createShareResource(input: {
  compareKind: ShareCompareKind;
  left: TextDocument;
  right: TextDocument | null;
  layout: { viewMode: 'graph' | 'text'; activePane: 'left' | 'right' };
  viewport: { left: ViewportAnchor; right: ViewportAnchor };
  interaction: ShareInteraction;
}): ShareResource {
  if (input.compareKind === 'none') {
    return { type: 'text_snapshot', payload: { schemaVersion: 1, left: input.left, right: input.right, layout: input.layout, interaction: input.interaction } };
  }
  if (!input.right) throw new Error('Compare shares require a right document.');
  return {
    type: 'compare',
    payload: {
      schemaVersion: 1,
      left: input.left,
      right: input.right,
      actions: [{ type: 'compare' }, { type: 'viewport_changed', payload: input.viewport }],
      interaction: input.interaction,
    },
  };
}
