import { ActionButton } from '@react-spectrum/s2';

export type DownloadRow = {
  id: string;
  title: string;
  status: string;
  progress: string;
  speed?: string;
  outputPath?: string | null;
  sessionLabel?: string;
};

type DownloadsTableProps = {
  mode: 'current' | 'history';
  rows: DownloadRow[];
  onOpenLocation: (row: DownloadRow) => void;
  onCancel?: (row: DownloadRow) => void;
  onDelete?: (row: DownloadRow) => void;
  emptyText: string;
  ariaLabel: string;
};

function formatStatus(status: string): string {
  switch (status) {
    case 'queued':
      return '排队中';
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
}: DownloadsTableProps) {
  if (rows.length === 0) {
    return <div className="dl-empty">{emptyText}</div>;
  }

  return (
    <ul className="dl-list" aria-label={ariaLabel}>
      {rows.map((row) => {
        const tone = statusTone(row.status);
        const percent = progressPercent(row.progress, row.status);

        return (
          <li key={row.id} className={`dl-row tone-${tone}`}>
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
  );
}
