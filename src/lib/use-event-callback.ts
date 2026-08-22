import { useCallback, useLayoutEffect, useRef } from 'react';

/**
 * Wrap an event handler so its identity never changes while it always runs the
 * latest closure. Handlers defined in the component body capture that render's
 * state, so passing them straight into a `React.memo` child defeats the memo —
 * every render hands the child a brand-new function.
 *
 * Only for event handlers: the stored closure is refreshed after render, so a
 * child that calls this during its own render can see the previous one.
 */
export function useEventCallback<Args extends unknown[], Result>(
  handler: (...args: Args) => Result,
): (...args: Args) => Result {
  const handlerRef = useRef(handler);

  useLayoutEffect(() => {
    handlerRef.current = handler;
  });

  return useCallback((...args: Args) => handlerRef.current(...args), []);
}
