import type { DownloadRow } from './DownloadsTable';

export type DownloadBucketState = {
  current: DownloadRow[];
  history: DownloadRow[];
};

const ACTIVE_STATUSES = ['queued', 'downloading', 'postprocessing', 'canceling'];

function isActiveStatus(status: string): boolean {
  return ACTIVE_STATUSES.includes(status);
}

export function createDownloadKey(sourceUrl: string, formatId?: string | null): string {
  return `${sourceUrl}::${formatId ?? ''}`;
}

function rowDownloadKey(row: DownloadRow): string | null {
  if (!row.sourceUrl) {
    return null;
  }
  return createDownloadKey(row.sourceUrl, row.formatId);
}

export function hasActiveDownloadForFormat(
  state: DownloadBucketState,
  sourceUrl: string,
  formatId?: string | null,
): boolean {
  const key = createDownloadKey(sourceUrl, formatId);
  return state.current.some((row) => rowDownloadKey(row) === key && isActiveStatus(row.status));
}

export function upsertCurrentDownloadRow(
  state: DownloadBucketState,
  row: DownloadRow,
): DownloadBucketState {
  const key = rowDownloadKey(row);
  const isSameTask = (item: DownloadRow) => item.id === row.id;
  const isSameDownload = (item: DownloadRow) => key !== null && rowDownloadKey(item) === key;

  return {
    current: [
      row,
      ...state.current.filter((item) => !isSameTask(item) && !isSameDownload(item)),
    ],
    history: state.history.filter((item) => !isSameTask(item) && !isSameDownload(item)),
  };
}
