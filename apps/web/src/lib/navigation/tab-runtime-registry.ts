import type { NavigationEntitySlices, TabNavigationStore, TabNavigationTarget, TabNavigationWriteResult } from './tab-navigation-store';

export type TabRuntimeBinding<Value> = {
  value: Value;
  dispose?: () => void;
};

export type TabRuntimeRegistration = TabNavigationWriteResult & {
  dispose(): void;
};

type StoredBinding = TabRuntimeBinding<unknown> & { target: TabNavigationTarget };

/** Keeps disposable UI/runtime resources out of persisted tab navigation state. */
export class TabRuntimeRegistry<Slices extends NavigationEntitySlices> {
  private readonly bindings = new Map<string, Map<string, StoredBinding>>();
  private readonly unsubscribeLifecycle: () => void;

  constructor(private readonly store: TabNavigationStore<Slices>) {
    this.unsubscribeLifecycle = store.subscribeLifecycle((event) => {
      if (event.kind === 'closed') this.disposeTarget(event.target);
      else this.disposeTarget(event.previous);
    });
  }

  register<Value>(target: TabNavigationTarget, key: string, binding: TabRuntimeBinding<Value>): TabRuntimeRegistration {
    const current = this.store.isCurrent(target);
    if (current.kind !== 'applied') return { ...current, dispose: () => {} };
    const tabBindings = this.bindings.get(target.tabId) ?? new Map<string, StoredBinding>();
    const previous = tabBindings.get(key);
    previous?.dispose?.();
    tabBindings.set(key, { ...binding, target });
    this.bindings.set(target.tabId, tabBindings);
    let disposed = false;
    return {
      kind: 'applied',
      dispose: () => {
        if (disposed) return;
        disposed = true;
        const currentBinding = this.bindings.get(target.tabId)?.get(key);
        if (currentBinding?.target !== target) return;
        this.bindings.get(target.tabId)?.delete(key);
        currentBinding.dispose?.();
      },
    };
  }

  get<Value>(target: TabNavigationTarget, key: string): Value | null {
    if (this.store.isCurrent(target).kind !== 'applied') return null;
    const binding = this.bindings.get(target.tabId)?.get(key);
    return binding?.target === target ? binding.value as Value : null;
  }

  disposeTarget(target: TabNavigationTarget): void {
    const tabBindings = this.bindings.get(target.tabId);
    if (!tabBindings) return;
    for (const [key, binding] of tabBindings) {
      if (binding.target.documentKey !== target.documentKey || binding.target.generation !== target.generation) continue;
      tabBindings.delete(key);
      binding.dispose?.();
    }
    if (tabBindings.size === 0) this.bindings.delete(target.tabId);
  }

  dispose(): void {
    this.unsubscribeLifecycle();
    for (const tabBindings of this.bindings.values()) {
      for (const binding of tabBindings.values()) binding.dispose?.();
    }
    this.bindings.clear();
  }
}
