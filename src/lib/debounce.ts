/**
 * Creates a debounced function that delays invoking `fn` until after `ms`
 * milliseconds have elapsed since the last time the debounced function was
 * invoked. Returns the debounced function with a `.cancel()` method.
 *
 * Replaces the es-toolkit `debounce` — keeps the same API surface.
 */
export function debounce<T extends (...args: never[]) => unknown>(fn: T, ms: number) {
  let timer: ReturnType<typeof setTimeout> | undefined;

  const debounced = (...args: Parameters<T>) => {
    if (timer !== undefined) clearTimeout(timer);
    timer = setTimeout(() => {
      timer = undefined;
      fn(...args);
    }, ms);
  };

  debounced.cancel = () => {
    if (timer !== undefined) {
      clearTimeout(timer);
      timer = undefined;
    }
  };

  return debounced;
}
