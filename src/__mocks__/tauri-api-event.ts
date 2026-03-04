type Callback<T> = (event: { payload: T }) => void;
type UnlistenFn = () => void;

const listeners = new Map<string, Set<Callback<unknown>>>();

export async function listen<T>(
  event: string,
  handler: Callback<T>
): Promise<UnlistenFn> {
  if (!listeners.has(event)) {
    listeners.set(event, new Set());
  }
  listeners.get(event)!.add(handler as Callback<unknown>);
  return () => {
    listeners.get(event)?.delete(handler as Callback<unknown>);
  };
}

export async function emit(_event: string, _payload?: unknown): Promise<void> {
  // no-op in Storybook
}

/** Emit a mock event to all registered listeners for testing */
export function __emitToListeners<T>(event: string, payload: T): void {
  listeners.get(event)?.forEach((cb) => cb({ payload }));
}

/** Clear all registered listeners */
export function __clearListeners(): void {
  listeners.clear();
}
