import { useEffect, useRef } from 'react';

export type ToastItem = {
  id: string;
  message: string;
  variant: 'info' | 'success' | 'error';
  durationMs?: number;
};

type ToastStackProps = {
  toasts: ToastItem[];
  onDismiss: (id: string) => void;
};

// Generous default so a toast is never missed; still auto-dismisses so none
// linger forever. Callers can override per toast.
const DEFAULT_DURATION_MS = 15000;

// Each toast owns its timer via a dedicated component. This is deliberate: a
// single shared effect keyed on the toasts array (plus an `onDismiss` whose
// identity changes every App render) would clear and recreate every timer on
// each re-render — and during downloads the app re-renders several times a
// second, so timers reset before firing and toasts never disappear.
function ToastCard({ toast, onDismiss }: { toast: ToastItem; onDismiss: (id: string) => void }) {
  const dismissRef = useRef(onDismiss);
  dismissRef.current = onDismiss;

  useEffect(() => {
    const duration = toast.durationMs ?? DEFAULT_DURATION_MS;
    const timer = window.setTimeout(() => dismissRef.current(toast.id), duration);
    return () => window.clearTimeout(timer);
  }, [toast.id, toast.durationMs]);

  return (
    <div className={`toast toast-${toast.variant}`} role="status">
      <span className="toast-message">{toast.message}</span>
      <button
        type="button"
        className="toast-close"
        aria-label="关闭提示"
        onClick={() => onDismiss(toast.id)}
      >
        ×
      </button>
    </div>
  );
}

export function ToastStack({ toasts, onDismiss }: ToastStackProps) {
  if (toasts.length === 0) {
    return null;
  }

  return (
    <div className="toast-stack" aria-live="polite" aria-atomic="true">
      {toasts.map((toast) => (
        <ToastCard key={toast.id} toast={toast} onDismiss={onDismiss} />
      ))}
    </div>
  );
}
