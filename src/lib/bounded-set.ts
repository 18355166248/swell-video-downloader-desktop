/**
 * Add to a set that must not grow for the lifetime of the session, evicting the
 * oldest entries once it passes `limit`.
 *
 * The sets this guards remember task ids — "already toasted", "user removed this
 * row" — so they only ever grow as downloads come and go. Sets iterate in
 * insertion order, so the oldest entry is simply the first one.
 *
 * Eviction is safe only because the entries stop mattering once their task is
 * long finished: keep `limit` well above the number of tasks that can plausibly
 * be in flight at once.
 */
export function addBounded(set: Set<string>, value: string, limit: number): void {
  set.add(value);

  if (limit <= 0) {
    set.clear();
    return;
  }

  while (set.size > limit) {
    const oldest = set.values().next();
    if (oldest.done) {
      return;
    }
    set.delete(oldest.value);
  }
}
