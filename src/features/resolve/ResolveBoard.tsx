import { ActionButton, Button, ProgressCircle, Text } from '@react-spectrum/s2';
import { useEffect, useRef, useState, type UIEvent } from 'react';
import { createPortal } from 'react-dom';
import { lockAppScroll } from '../../lib/scroll-lock';
import type { MediaFormat, ResolveMediaResponse } from '../../lib/types';

export type ResolveItemStatus = 'loading' | 'ready' | 'failed' | 'selected';

export type ResolveItem = {
  url: string;
  status: ResolveItemStatus;
  resolved: ResolveMediaResponse | null;
  thumbnail: string | null;
  isPreviewLoading: boolean;
  error?: string;
  selectedLabel?: string;
};

type ResolveBoardProps = {
  items: ResolveItem[];
  openUrl: string | null;
  downloadingIdsFor: (url: string) => ReadonlySet<string>;
  onOpenChange: (url: string | null) => void;
  onDownload: (item: ResolveItem, format: MediaFormat) => void;
};

// Scroll distance (px) over which the drawer preview shrinks to its minimum.
const PREVIEW_SHRINK_DISTANCE = 200;

// Max cards shown per page in the resolve grid; beyond this, pagination kicks in.
const GRID_PAGE_SIZE = 12;

const STATUS_TEXT: Record<ResolveItemStatus, string> = {
  loading: '解析中',
  ready: '待选择',
  selected: '已选择',
  failed: '失败',
};

function formatBytes(bytes?: number | null): string {
  if (!bytes || bytes <= 0) {
    return '未知大小';
  }
  const units = ['B', 'KB', 'MB', 'GB'];
  let value = bytes;
  let unitIndex = 0;
  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }
  return `${value.toFixed(value >= 100 || unitIndex === 0 ? 0 : 1)} ${units[unitIndex]}`;
}

function hostnameOf(url: string): string {
  try {
    return new URL(url).hostname.replace(/^www\./u, '');
  } catch {
    return url;
  }
}

function titleOf(item: ResolveItem): string {
  return item.resolved?.title ?? hostnameOf(item.url);
}

function badgeText(item: ResolveItem): string {
  if (item.status === 'selected' && item.selectedLabel) {
    return `已选 · ${item.selectedLabel}`;
  }
  return STATUS_TEXT[item.status];
}

export function ResolveBoard({
  items,
  openUrl,
  downloadingIdsFor,
  onOpenChange,
  onDownload,
}: ResolveBoardProps) {
  // Only resolved/failed rows can open; loading tiles aren't interactive yet.
  const openable = items.filter((item) => item.status !== 'loading');
  const openItem = items.find((item) => item.url === openUrl && item.status !== 'loading') ?? null;
  const openIndex = openItem ? openable.findIndex((item) => item.url === openItem.url) : -1;
  const prevItem = openIndex > 0 ? openable[openIndex - 1] : null;
  const nextItem =
    openIndex >= 0 && openIndex < openable.length - 1 ? openable[openIndex + 1] : null;

  // Pagination for the grid: show GRID_PAGE_SIZE cards per page.
  const gridPageCount = Math.max(1, Math.ceil(items.length / GRID_PAGE_SIZE));
  const [gridPage, setGridPage] = useState(0);

  // Clamp page when items shrink (e.g. resolve cancelled mid-flight).
  useEffect(() => {
    setGridPage((current) => Math.min(current, gridPageCount - 1));
  }, [gridPageCount]);

  // Auto-navigate to the page containing the opened item so the highlight is
  // always visible when a drawer opens.
  useEffect(() => {
    if (!openUrl) {
      return;
    }
    const idx = items.findIndex((item) => item.url === openUrl);
    if (idx >= 0) {
      setGridPage(Math.floor(idx / GRID_PAGE_SIZE));
    }
  }, [openUrl, items]);

  const safeGridPage = Math.min(gridPage, gridPageCount - 1);
  const needsPagination = items.length > GRID_PAGE_SIZE;
  const pageItems = needsPagination
    ? items.slice(safeGridPage * GRID_PAGE_SIZE, safeGridPage * GRID_PAGE_SIZE + GRID_PAGE_SIZE)
    : items;
  // Global index offset so card numbering stays sequential across pages.
  const pageOffset = needsPagination ? safeGridPage * GRID_PAGE_SIZE : 0;

  const bodyRef = useRef<HTMLDivElement>(null);

  // Drive the sticky preview's shrink amount from the drawer scroll position.
  function handleDrawerScroll(event: UIEvent<HTMLDivElement>) {
    const el = event.currentTarget;
    const progress = Math.min(1, Math.max(0, el.scrollTop / PREVIEW_SHRINK_DISTANCE));
    el.style.setProperty('--preview-shrink', String(progress));
  }

  // Reset scroll + preview size whenever a different video is opened.
  useEffect(() => {
    const el = bodyRef.current;
    if (!el) {
      return;
    }
    el.scrollTop = 0;
    el.style.setProperty('--preview-shrink', '0');
  }, [openItem?.url]);

  const isDrawerOpen = openItem !== null;
  useEffect(() => {
    if (!isDrawerOpen) {
      return;
    }
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        onOpenChange(null);
      }
    };
    window.addEventListener('keydown', onKey);
    // Freeze the page behind the drawer (shared lock so nested overlays and
    // re-renders can't leave the page stuck with overflow: hidden).
    const releaseScroll = lockAppScroll();
    return () => {
      window.removeEventListener('keydown', onKey);
      releaseScroll();
    };
  }, [isDrawerOpen, onOpenChange]);

  return (
    <>
      <div className="resolve-grid">
        {pageItems.map((item, pageIndex) => {
          const globalIndex = pageOffset + pageIndex;
          const interactive = item.status !== 'loading';
          return (
            <button
              key={item.url}
              type="button"
              className={`resolve-entry is-${item.status}${item.url === openUrl ? ' is-open' : ''}`}
              onClick={() => (interactive ? onOpenChange(item.url) : undefined)}
              disabled={!interactive}
              title={titleOf(item)}
            >
              <span className="resolve-entry-top">
                <span className={`resolve-dot dot-${item.status}`} aria-hidden="true" />
                <span className="resolve-entry-index">{String(globalIndex + 1).padStart(2, '0')}</span>
                <span className={`resolve-entry-status badge-${item.status}`}>
                  {item.status === 'loading' ? (
                    <ProgressCircle aria-label="解析中" size="S" isIndeterminate />
                  ) : null}
                  {badgeText(item)}
                </span>
              </span>
              <span className="resolve-entry-title">{titleOf(item)}</span>
            </button>
          );
        })}
      </div>

      {needsPagination ? (
        <div className="resolve-pager">
          <ActionButton
            aria-label="上一页"
            isDisabled={safeGridPage === 0}
            onPress={() => setGridPage((current) => Math.max(0, current - 1))}
          >
            上一页
          </ActionButton>
          <span className="resolve-pager-info">
            第 {safeGridPage + 1} / {gridPageCount} 页 · 共 {items.length} 个
          </span>
          <ActionButton
            aria-label="下一页"
            isDisabled={safeGridPage >= gridPageCount - 1}
            onPress={() => setGridPage((current) => Math.min(gridPageCount - 1, current + 1))}
          >
            下一页
          </ActionButton>
        </div>
      ) : null}

      {openItem
        ? createPortal(
          <>
          <div
            className="drawer-backdrop"
            role="presentation"
            onClick={() => onOpenChange(null)}
          />
          <aside className="drawer" aria-label="选择清晰度">
            <header className="drawer-head">
              <div className="drawer-head-main">
                <span className={`resolve-entry-status badge-${openItem.status}`}>
                  {badgeText(openItem)}
                </span>
                <Text UNSAFE_className="drawer-title">{titleOf(openItem)}</Text>
              </div>
              <div className="drawer-nav">
                <button
                  type="button"
                  className="drawer-nav-btn"
                  aria-label="上一个视频"
                  disabled={!prevItem}
                  onClick={() => prevItem && onOpenChange(prevItem.url)}
                >
                  ‹
                </button>
                <span className="drawer-nav-count">
                  {openIndex + 1} / {openable.length}
                </span>
                <button
                  type="button"
                  className="drawer-nav-btn"
                  aria-label="下一个视频"
                  disabled={!nextItem}
                  onClick={() => nextItem && onOpenChange(nextItem.url)}
                >
                  ›
                </button>
                <button
                  type="button"
                  className="drawer-close"
                  aria-label="关闭"
                  onClick={() => onOpenChange(null)}
                >
                  ✕
                </button>
              </div>
            </header>

            <div className="drawer-body" ref={bodyRef} onScroll={handleDrawerScroll}>
              {openItem.status === 'failed' ? (
                <Text UNSAFE_className="resolve-error">{openItem.error ?? '解析失败'}</Text>
              ) : openItem.resolved ? (
                <>
                  <div className="preview-sticky">
                    <div className="preview-frame">
                      {openItem.thumbnail ? (
                        <img className="preview-image" src={openItem.thumbnail} alt="视频预览" />
                      ) : (
                        <div className="preview-placeholder">
                          <Text>{openItem.isPreviewLoading ? '正在生成预览…' : '无预览'}</Text>
                        </div>
                      )}
                    </div>
                  </div>

                  <div className="result-meta">
                    <Text UNSAFE_className="meta-tag">{openItem.resolved.source}</Text>
                    <Text UNSAFE_className="meta-tag">时长 {openItem.resolved.durationText}</Text>
                    <Text UNSAFE_className="meta-tag">
                      {openItem.resolved.formats.length} 个版本
                    </Text>
                  </div>

                  <Text UNSAFE_className="format-list-title">选择要下载的版本</Text>
                  <ul className="format-list">
                    {openItem.resolved.formats.map((format) => (
                      <li
                        key={format.id}
                        className={`format-row${
                          downloadingIdsFor(openItem.url).has(format.id) ? ' is-submitting' : ''
                        }`}
                      >
                        <div className="format-info">
                          <Text UNSAFE_className="format-label">{format.label}</Text>
                          <Text UNSAFE_className="format-sub">
                            {format.ext.toUpperCase()} · {formatBytes(format.sizeBytes)}
                            {format.hasAudio ? ' · 含音轨' : ' · 无音轨'}
                          </Text>
                        </div>
                        <Button
                          variant="accent"
                          onPress={() => onDownload(openItem, format)}
                          isPending={downloadingIdsFor(openItem.url).has(format.id)}
                        >
                          {downloadingIdsFor(openItem.url).has(format.id) ? '加入中…' : '下载'}
                        </Button>
                      </li>
                    ))}
                  </ul>
                </>
              ) : null}
            </div>
          </aside>
          </>,
          document.body,
        )
        : null}
    </>
  );
}
