import { Button, Text } from '@react-spectrum/s2';
import { useEffect, useState } from 'react';
import { createPortal } from 'react-dom';
import { lockAppScroll } from '../../lib/scroll-lock';
import sessionIdGuide from '../../assets/getigsessionid.png';

const STEPS = [
  '在电脑上打开一个浏览器',
  '访问 Instagram 并登录',
  '在页面上点右键选择「检查 / Inspect」（或直接按 F12）',
  '（如下图所示）切换到「Application 应用」标签页',
  '在左侧选择「Cookies」',
  '找到「sessionid」对应的值并复制',
  '回到本页面，粘贴「sessionid」值，输入 Instagram 帖子链接，点击下载',
];

export function SessionIdHelpDrawer() {
  const [open, setOpen] = useState(false);
  const [zoomed, setZoomed] = useState(false);

  useEffect(() => {
    if (!open) {
      return;
    }
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        // Escape first closes the zoom, then the drawer.
        setZoomed((current) => {
          if (current) {
            return false;
          }
          setOpen(false);
          return current;
        });
      }
    };
    window.addEventListener('keydown', onKey);
    const releaseScroll = lockAppScroll();
    return () => {
      window.removeEventListener('keydown', onKey);
      releaseScroll();
    };
  }, [open]);

  // Reset the zoom state whenever the drawer closes.
  useEffect(() => {
    if (!open) {
      setZoomed(false);
    }
  }, [open]);

  return (
    <>
      <Button variant="secondary" onPress={() => setOpen(true)}>
        如何获取 sessionid？
      </Button>

      {open
        ? createPortal(
            <>
              <div
                className="drawer-backdrop"
                role="presentation"
                onClick={() => setOpen(false)}
              />
              <aside className="drawer sessionid-drawer" aria-label="如何获取 sessionid">
                <header className="drawer-head">
                  <div className="drawer-head-main">
                    <Text UNSAFE_className="drawer-title">如何获取 Cookie（sessionid）</Text>
                  </div>
                  <div className="drawer-nav">
                    <button
                      type="button"
                      className="drawer-close"
                      aria-label="关闭"
                      onClick={() => setOpen(false)}
                    >
                      ✕
                    </button>
                  </div>
                </header>

                <div className="drawer-body">
                  <ol className="sessionid-steps">
                    {STEPS.map((step) => (
                      <li key={step}>{step}</li>
                    ))}
                  </ol>

                  <button
                    type="button"
                    className="sessionid-guide-button"
                    onClick={() => setZoomed(true)}
                    aria-label="点击放大查看示意图"
                  >
                    <img
                      className="sessionid-guide-image"
                      src={sessionIdGuide}
                      alt="在浏览器开发者工具的 Application → Cookies 中找到 sessionid"
                    />
                    <span className="sessionid-guide-zoom-hint">点击放大 ⤢</span>
                  </button>

                  <Text UNSAFE_className="sessionid-privacy-note">
                    你的 Cookie（sessionid）只会保存在本机设备上。
                  </Text>
                </div>
              </aside>

              {zoomed ? (
                <div
                  className="sessionid-lightbox"
                  role="presentation"
                  onClick={() => setZoomed(false)}
                >
                  <img
                    className="sessionid-lightbox-image"
                    src={sessionIdGuide}
                    alt="在浏览器开发者工具的 Application → Cookies 中找到 sessionid（放大）"
                  />
                  <button
                    type="button"
                    className="sessionid-lightbox-close"
                    aria-label="关闭放大"
                    onClick={() => setZoomed(false)}
                  >
                    ✕
                  </button>
                </div>
              ) : null}
            </>,
            document.body,
          )
        : null}
    </>
  );
}
