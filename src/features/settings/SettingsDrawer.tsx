import { Text } from '@react-spectrum/s2';
import { useEffect } from 'react';
import { createPortal } from 'react-dom';
import { lockAppScroll } from '../../lib/scroll-lock';
import { SettingsPanel, type SettingsPanelProps } from './SettingsPanel';

type SettingsDrawerProps = SettingsPanelProps & {
  open: boolean;
  onClose: () => void;
};

export function SettingsDrawer({ open, onClose, ...panelProps }: SettingsDrawerProps) {
  useEffect(() => {
    if (!open) {
      return;
    }
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        onClose();
      }
    };
    window.addEventListener('keydown', onKey);
    const releaseScroll = lockAppScroll();
    return () => {
      window.removeEventListener('keydown', onKey);
      releaseScroll();
    };
  }, [open, onClose]);

  if (!open) {
    return null;
  }

  return createPortal(
    <>
      <div className="drawer-backdrop" role="presentation" onClick={onClose} />
      <aside className="drawer settings-drawer" aria-label="设置">
        <header className="drawer-head">
          <div className="drawer-head-main">
            <Text UNSAFE_className="drawer-title">设置</Text>
          </div>
          <div className="drawer-nav">
            <button
              type="button"
              className="drawer-close"
              aria-label="关闭设置"
              onClick={onClose}
            >
              ✕
            </button>
          </div>
        </header>
        <div className="drawer-body">
          <SettingsPanel {...panelProps} />
        </div>
      </aside>
    </>,
    document.body,
  );
}
