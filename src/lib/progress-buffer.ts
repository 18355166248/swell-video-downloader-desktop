type TaskKeyed = { task_id: string };

export type ProgressBuffer<T extends TaskKeyed> = {
  /** Queue a tick. Only the newest tick per task survives a flush window. */
  push: (payload: T) => void;
  /** Forget a task's queued tick — it finished, or its row was removed. */
  drop: (taskId: string) => void;
  /** Deliver whatever is queued right now. */
  flush: () => void;
  /** Drop everything queued and stop the timer. */
  cancel: () => void;
};

/**
 * Collect progress ticks and hand them over in batches.
 *
 * The backend reports progress per output line, which is far more often than a
 * UI needs to repaint: without buffering, eight parallel downloads turn into
 * dozens of state updates a second. Ticks are keyed by task, so a window only
 * ever yields each task's latest position.
 */
export function createProgressBuffer<T extends TaskKeyed>(
  flushMs: number,
  onFlush: (batch: T[]) => void,
): ProgressBuffer<T> {
  const pending = new Map<string, T>();
  let timer: ReturnType<typeof setTimeout> | null = null;

  function clearTimer() {
    if (timer !== null) {
      clearTimeout(timer);
      timer = null;
    }
  }

  function flush() {
    clearTimer();
    if (pending.size === 0) {
      return;
    }
    const batch = [...pending.values()];
    pending.clear();
    onFlush(batch);
  }

  return {
    push(payload) {
      pending.set(payload.task_id, payload);
      if (timer === null) {
        timer = setTimeout(flush, flushMs);
      }
    },
    drop(taskId) {
      pending.delete(taskId);
    },
    flush,
    cancel() {
      clearTimer();
      pending.clear();
    },
  };
}
