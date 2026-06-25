export const DEFAULT_DOWNLOAD_CONCURRENCY = 3;
export const MIN_DOWNLOAD_CONCURRENCY = 1;
export const MAX_DOWNLOAD_CONCURRENCY = 8;

export function normalizeDownloadConcurrency(value: unknown): number {
  if (typeof value !== 'number' || !Number.isFinite(value)) {
    return DEFAULT_DOWNLOAD_CONCURRENCY;
  }

  return Math.min(
    MAX_DOWNLOAD_CONCURRENCY,
    Math.max(MIN_DOWNLOAD_CONCURRENCY, Math.floor(value)),
  );
}
