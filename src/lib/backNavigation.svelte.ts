
export type BackHandler = {
  id: string;
  priority: number;
  canGoBack: () => boolean;
  goBack: () => void;
  label?: string | (() => string);
};

let handlers = $state<BackHandler[]>([]);

function sortHandlers(list: BackHandler[]) {
  return [...list].sort((a, b) => b.priority - a.priority);
}

export function registerBackHandler(handler: BackHandler): () => void {
  handlers = sortHandlers([...handlers.filter((h) => h.id !== handler.id), handler]);
  return () => {
    handlers = handlers.filter((h) => h.id !== handler.id);
  };
}

export function getBackHandlers() {
  return handlers;
}

export function getActiveBackHandler(): BackHandler | null {
  return handlers.find((h) => h.canGoBack()) ?? null;
}

export function canNavigateBack(): boolean {
  return handlers.some((h) => h.canGoBack());
}

export function navigateBack(): boolean {
  const active = getActiveBackHandler();
  if (!active) return false;
  active.goBack();
  return true;
}

export function getBackDescription(fallback?: string): string {
  const defaultFallback = fallback ?? "Undo last action";
  const active = getActiveBackHandler();
  if (!active?.label) return defaultFallback;
  return typeof active.label === 'function' ? active.label() : active.label;
}
