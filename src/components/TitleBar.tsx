import { getCurrentWindow } from '@tauri-apps/api/window';
import { useEffect, useState } from 'react';

// Mirrors the (unexported) ResizeDirection union from @tauri-apps/api/window.
type ResizeDirection =
  | 'North'
  | 'South'
  | 'East'
  | 'West'
  | 'NorthEast'
  | 'NorthWest'
  | 'SouthEast'
  | 'SouthWest';

const appWindow = getCurrentWindow();

// Invisible edge/corner grips that re-enable resizing on an undecorated window.
const RESIZE_HANDLES: Array<{ className: string; direction: ResizeDirection }> = [
  { className: 'resize-n', direction: 'North' },
  { className: 'resize-s', direction: 'South' },
  { className: 'resize-e', direction: 'East' },
  { className: 'resize-w', direction: 'West' },
  { className: 'resize-ne', direction: 'NorthEast' },
  { className: 'resize-nw', direction: 'NorthWest' },
  { className: 'resize-se', direction: 'SouthEast' },
  { className: 'resize-sw', direction: 'SouthWest' },
];

export function TitleBar() {
  const [isMaximized, setIsMaximized] = useState(false);

  useEffect(() => {
    let dispose: (() => void) | undefined;

    appWindow.isMaximized().then(setIsMaximized).catch(() => undefined);
    appWindow
      .onResized(() => {
        appWindow.isMaximized().then(setIsMaximized).catch(() => undefined);
      })
      .then((unlisten) => {
        dispose = unlisten;
      })
      .catch(() => undefined);

    return () => dispose?.();
  }, []);

  return (
    <>
      <div className="resize-layer">
        {RESIZE_HANDLES.map((handle) => (
          <div
            key={handle.className}
            className={`resize-handle ${handle.className}`}
            onMouseDown={(event) => {
              if (event.button !== 0) {
                return;
              }
              void appWindow.startResizeDragging(handle.direction);
            }}
          />
        ))}
      </div>

      <header className="titlebar" data-tauri-drag-region>
        <div className="titlebar-brand" data-tauri-drag-region>
          <img
            src="/app-icon.png"
            alt=""
            className="brand-icon"
            aria-hidden="true"
            draggable={false}
          />
          <span className="brand-name">
            Swell <em>Video</em> Downloader
          </span>
        </div>

        <div className="titlebar-controls">
          <button
            type="button"
            className="win-btn"
            aria-label="最小化"
            onClick={() => void appWindow.minimize()}
          >
            <svg width="11" height="11" viewBox="0 0 11 11" aria-hidden="true">
              <line x1="1.5" y1="5.5" x2="9.5" y2="5.5" />
            </svg>
          </button>

          <button
            type="button"
            className="win-btn"
            aria-label={isMaximized ? '还原' : '最大化'}
            onClick={() => void appWindow.toggleMaximize()}
          >
            {isMaximized ? (
              <svg width="11" height="11" viewBox="0 0 11 11" aria-hidden="true">
                <rect x="1.5" y="3" width="6" height="6" rx="0.8" />
                <path d="M3.5 3V1.5h6v6H8" fill="none" />
              </svg>
            ) : (
              <svg width="11" height="11" viewBox="0 0 11 11" aria-hidden="true">
                <rect x="1.5" y="1.5" width="8" height="8" rx="0.8" />
              </svg>
            )}
          </button>

          <button
            type="button"
            className="win-btn win-btn-close"
            aria-label="关闭"
            onClick={() => void appWindow.close()}
          >
            <svg width="11" height="11" viewBox="0 0 11 11" aria-hidden="true">
              <line x1="1.5" y1="1.5" x2="9.5" y2="9.5" />
              <line x1="9.5" y1="1.5" x2="1.5" y2="9.5" />
            </svg>
          </button>
        </div>
      </header>
    </>
  );
}
