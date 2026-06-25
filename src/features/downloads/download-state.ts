import type { DownloadRow } from './DownloadsTable';

export type DownloadBucketState = {
  current: DownloadRow[];
  history: DownloadRow[];
};

export type DownloadProgressUpdate = {
  taskId: string;
  percent: string;
  speed: string;
};

export type DownloadStatusUpdate = {
  taskId: string;
  title: string;
  status: string;
  outputPath?: string | null;
};

export const DOWNLOAD_HISTORY_LIMIT = 100;

const ACTIVE_STATUSES = ['starting', 'queued', 'downloading', 'postprocessing', 'canceling'];

export function isActiveStatus(status: string): boolean {
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

export function pruneDownloadHistory(
  history: DownloadRow[],
  limit = DOWNLOAD_HISTORY_LIMIT,
): DownloadRow[] {
  return history.slice(0, Math.max(0, limit));
}

export function mergeRowsById(primary: DownloadRow[], secondary: DownloadRow[]): DownloadRow[] {
  const seen = new Set<string>();
  const merged: DownloadRow[] = [];

  for (const row of [...primary, ...secondary]) {
    if (seen.has(row.id)) {
      continue;
    }
    seen.add(row.id);
    merged.push(row);
  }

  return merged;
}

export function createFallbackRow(
  taskId: string,
  title: string,
  status: string,
): DownloadRow {
  return {
    id: taskId,
    title,
    sessionLabel: '后台任务',
    status,
    progress: status === 'completed' ? '100%' : '0%',
  };
}

export function archiveFinishedRows(state: DownloadBucketState): DownloadBucketState {
  const finished = state.current.filter((row) => !isActiveStatus(row.status));
  if (finished.length === 0) {
    return {
      ...state,
      history: pruneDownloadHistory(state.history),
    };
  }

  return {
    current: state.current.filter((row) => isActiveStatus(row.status)),
    history: pruneDownloadHistory(mergeRowsById(finished, state.history)),
  };
}

function updateRowCollections(
  state: DownloadBucketState,
  taskId: string,
  updater: (row: DownloadRow) => DownloadRow,
  fallbackRow: DownloadRow,
): DownloadBucketState {
  const updateList = (rows: DownloadRow[]) =>
    rows.map((row) => (row.id === taskId ? updater(row) : row));

  const inCurrent = state.current.some((row) => row.id === taskId);
  const inHistory = state.history.some((row) => row.id === taskId);

  if (inCurrent || inHistory) {
    return {
      current: inCurrent ? updateList(state.current) : state.current,
      history: pruneDownloadHistory(inHistory ? updateList(state.history) : state.history),
    };
  }

  return {
    ...state,
    current: [fallbackRow, ...state.current],
    history: pruneDownloadHistory(state.history),
  };
}

export function updateDownloadProgress(
  state: DownloadBucketState,
  update: DownloadProgressUpdate,
): DownloadBucketState {
  return updateRowCollections(
    state,
    update.taskId,
    (row) => ({
      ...row,
      progress: update.percent,
      speed: update.speed,
    }),
    {
      ...createFallbackRow(update.taskId, '历史任务', 'downloading'),
      progress: update.percent,
      speed: update.speed,
    },
  );
}

export function updateDownloadStatus(
  state: DownloadBucketState,
  update: DownloadStatusUpdate,
): DownloadBucketState {
  const updated = updateRowCollections(
    state,
    update.taskId,
    (row) => ({
      ...row,
      title: update.title,
      status: update.status,
      progress:
        update.status === 'completed'
          ? '100%'
          : update.status === 'failed'
            ? row.progress || '0%'
            : row.progress,
      outputPath: update.outputPath ?? row.outputPath,
    }),
    {
      ...createFallbackRow(update.taskId, update.title, update.status),
      outputPath: update.outputPath ?? null,
    },
  );

  if (
    update.status === 'completed' ||
    update.status === 'failed' ||
    update.status === 'canceled'
  ) {
    return archiveFinishedRows(updated);
  }

  return updated;
}

export function deleteHistoryRow(
  state: DownloadBucketState,
  rowId: string,
): DownloadBucketState {
  return {
    ...state,
    history: state.history.filter((item) => item.id !== rowId),
  };
}

export function removeCurrentRow(
  state: DownloadBucketState,
  rowId: string,
): DownloadBucketState {
  return {
    ...state,
    current: state.current.filter((item) => item.id !== rowId),
  };
}

export function replaceCurrentDownloadRow(
  state: DownloadBucketState,
  previousRowId: string,
  row: DownloadRow,
): DownloadBucketState {
  const next = upsertCurrentDownloadRow(
    {
      ...state,
      current: state.current.filter((item) => item.id !== previousRowId),
    },
    row,
  );

  return {
    ...next,
    history: pruneDownloadHistory(next.history),
  };
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
    history: pruneDownloadHistory(
      state.history.filter((item) => !isSameTask(item) && !isSameDownload(item)),
    ),
  };
}
