// Shared scroll lock for the app's scroll region (`.app-scroll`). Multiple
// overlays (drawers, lightboxes) can be open or nested at once; each acquires a
// lock and the region only becomes scrollable again once every lock is released.
// This avoids the per-overlay "save & restore previous overflow" races that could
// leave the page stuck with `overflow: hidden` after closing.

let lockCount = 0;

function appScroller(): HTMLElement | null {
  return document.querySelector<HTMLElement>('.app-scroll');
}

/**
 * Lock the app scroll region. Returns a release function that is safe to call
 * once; the region is restored only when the last lock is released.
 */
export function lockAppScroll(): () => void {
  lockCount += 1;
  const el = appScroller();
  if (el) {
    el.style.overflow = 'hidden';
  }

  let released = false;
  return () => {
    if (released) {
      return;
    }
    released = true;
    lockCount = Math.max(0, lockCount - 1);
    if (lockCount === 0) {
      const current = appScroller();
      if (current) {
        current.style.overflow = '';
      }
    }
  };
}
