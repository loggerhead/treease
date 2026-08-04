import type { NavigationBehavior, NavigationBehaviorPolicy, NavigationSettings, NavigationUserEvent } from './navigation-contract';

export const navigationBehaviorPolicy: NavigationBehaviorPolicy = {
  decide(event, settings) {
    switch (event.kind) {
      case 'editor-selection':
      case 'graph-cell':
      case 'navigator-column':
        return settings.completeNavigationEnabled ? 'navigate' : 'locate';
      case 'navigator-tree-path':
      case 'search-commit':
        return 'navigate';
      case 'search-preview':
        return settings.completeNavigationEnabled ? 'graph-viewport-preview' : 'graph-highlight-preview';
      case 'search-cancel':
        return 'cancel-preview';
      case 'graph-viewport-gesture':
      case 'editor-edit':
      case 'editor-scroll':
      case 'tab-activated':
      case 'state-restored':
        return 'none';
    }
  },
};

export function decideNavigationBehavior(
  event: NavigationUserEvent,
  settings: NavigationSettings,
): NavigationBehavior {
  return navigationBehaviorPolicy.decide(event, settings);
}
