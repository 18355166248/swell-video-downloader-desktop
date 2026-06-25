import { ActionButton } from '@react-spectrum/s2';
import { useEffect, useState } from 'react';

const PAGE_SIZE = 6;

export type DownloadRow = {
  id: string;
  title: string;
  status: string;
  progress: string;
  speed?: string;
  outputPath?: string | null;
  sessionLabel?: string;
  sourceUrl?: string;
  formatId?: string | null;
};

type DownloadsTableProps = {
  mode: 'current' | 'history';
  rows: DownloadRow[];
  onOpenLocation: (row: DownloadRow) => void;
  onCancel?: (row: DownloadRow) => void;
  onDelete?: (row: DownloadRow) => void;
  emptyText: string;
  ariaLabel: string;
  /** When true, show checkboxes for batch selection (used by the retry tab). */
  selectable?: boolean;
  selectedIds?: Set<string>;
  onSelectionChange?: (ids: Set<string>) => void;
};

function formatStatus(status: string): string {
  switch (status) {
    case 'queued':
      return '排队中';
    case 'starting':
      return '加入队列中';
    case 'downloading':
      return '下载中';
    case 'postprocessing':
      return '处理中';
    case 'completed':
      return '已完成';
    case 'failed':
      return '失败';
    case 'canceling':
      return '取消中';
    case 'canceled':
      return '已取消';
    default:
      return status;
  }
}

// Map a status to a coarse tone used for the badge + progress-bar colour.
function statusTone(status: string): 'active' | 'done' | 'failed' | 'idle' {
  switch (status) {
    case 'downloading':
    case 'postprocessing':
    case 'starting':
      return 'active';
    case 'completed':
      return 'done';
    case 'failed':
    case 'canceled':
      return 'failed';
    default:
      return 'idle';
  }
}

function canCancel(status: string) {
  return ['queued', 'downloading', 'postprocessing', 'canceling'].includes(status);
}

// "93.5%" / "0%" → clamped number for the bar width; defaults to 0 when unknown.
function progressPercent(progress: string, status: string): number {
  if (status === 'completed') {
    return 100;
  }
  const value = Number.parseFloat(progress);
  if (Number.isNaN(value)) {
    return 0;
  }
  return Math.max(0, Math.min(100, value));
}

export function DownloadsTable({
  mode,
  rows,
  onOpenLocation,
  onCancel,
  onDelete,
  emptyText,
  ariaLabel,
  selectable,
  selectedIds,
  onSelectionChange,
}: DownloadsTableProps) {
  const pageCount = Math.max(1, Math.ceil(rows.length / PAGE_SIZE));
  const [page, setPage] = useState(0);
  // Clamp the page when the row count shrinks (e.g. a row is deleted/archived).
  useEffect(() => {
    setPage((current) => Math.min(current, pageCount - 1));
  }, [pageCount]);

  if (rows.length === 0) {
    return <div className="dl-empty">{emptyText}</div>;
  }

  const safePage = Math.min(page, pageCount - 1);
  const pageRows = rows.slice(safePage * PAGE_SIZE, safePage * PAGE_SIZE + PAGE_SIZE);

  const allPageIds = pageRows.map((r) => r.id);
  const allSelected = allPageIds.length > 0 && allPageIds.every((id) => selectedIds?.has(id));
  const someSelected = allPageIds.some((id) => selectedIds?.has(id));

  function toggleSelectAll() {
    if (!onSelectionChange || !selectedIds) return;
    if (allSelected) {
      // Deselect all on this page
      const next = new Set(selectedIds);
      for (const id of allPageIds) next.delete(id);
      onSelectionChange(next);
    } else {
      // Select all on this page
      const next = new Set(selectedIds);
      for (const id of allPageIds) next.add(id);
      onSelectionChange(next);
    }
  }

  function toggleRow(id: string) {
    if (!onSelectionChange || !selectedIds) return;
    const next = new Set(selectedIds);
    if (next.has(id)) {
      next.delete(id);
    } else {
      next.add(id);
    }
    onSelectionChange(next);
  }

  return (
    <>
    {selectable ? (
      <div className="dl-select-bar">
        <label className="dl-select-all">
          <input
            type="checkbox"
            checked={allSelected}
            ref={(el) => { if (el) el.indeterminate = someSelected && !allSelected; }}
            onChange={toggleSelectAll}
          />
          <span>
            {selectedIds && selectedIds.size > 0
              ? `已选 ${selectedIds.size} / ${rows.length}`
              : `全选（${rows.length}）`}
          </span>
        </label>
      </div>
    ) : null}
    <ul className="dl-list" aria-label={ariaLabel}>
      {pageRows.map((row) => {
        const tone = statusTone(row.status);
        const percent = progressPercent(row.progress, row.status);

        return (
          <li key={row.id} className={`dl-row tone-${tone}${selectable && selectedIds?.has(row.id) ? ' is-selected' : ''}`}>
            {selectable ? (
              <label className="dl-row-checkbox">
                <input
                  type="checkbox"
                  checked={selectedIds?.has(row.id) ?? false}
                  onChange={() => toggleRow(row.id)}
                />
              </label>
            ) : null}
            <div className="dl-main">
              <span className="dl-title" title={row.title}>
                {row.title}
              </span>
              <div className="dl-meta">
                <span className={`dl-status tone-${tone}`}>{formatStatus(row.status)}</span>
                {row.sessionLabel ? <span className="dl-meta-sep">{row.sessionLabel}</span> : null}
                {row.speed ? <span className="dl-meta-sep">{row.speed}</span> : null}
              </div>
            </div>

            <div className="dl-progress" aria-label={`进度 ${row.progress}`}>
              <div className="dl-bar">
                <span className="dl-bar-fill" style={{ width: `${percent}%` }} />
              </div>
              <span className="dl-pct">{row.progress}</span>
            </div>

            <div className="dl-actions">
              <ActionButton
                aria-label={row.outputPath ? '打开所在文件夹' : '打开下载目录'}
                onPress={() => onOpenLocation(row)}
              >
                打开目录
              </ActionButton>
              {mode === 'current' && onCancel && canCancel(row.status) ? (
                <ActionButton aria-label="取消下载" onPress={() => onCancel(row)}>
                  取消
                </ActionButton>
              ) : null}
              {onDelete ? (
                <ActionButton
                  aria-label={mode === 'history' ? '删除记录' : '从队列移除'}
                  onPress={() => onDelete(row)}
                >
                  删除
                </ActionButton>
              ) : null}
            </div>
          </li>
        );
      })}
    </ul>
    {pageCount > 1 ? (
      <div className="dl-pager">
        <ActionButton
          aria-label="上一页"
          isDisabled={safePage === 0}
          onPress={() => setPage((current) => Math.max(0, current - 1))}
        >
          上一页
        </ActionButton>
        <span className="dl-pager-info">
          第 {safePage + 1} / {pageCount} 页 · 共 {rows.length} 条
        </span>
        <ActionButton
          aria-label="下一页"
          isDisabled={safePage >= pageCount - 1}
          onPress={() => setPage((current) => Math.min(pageCount - 1, current + 1))}
        >
          下一页
        </ActionButton>
      </div>
    ) : null}
    </>
  );
}
