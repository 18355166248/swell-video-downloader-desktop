import { listen } from '@tauri-apps/api/event';

export type DownloadProgressPayload = {
  task_id: string;
  percent: string;
  speed: string;
  eta: string;
};

export type DownloadStatusPayload = {
  task_id: string;
  title: string;
  status: string;
  message?: string | null;
  output_path?: string | null;
};

export function listenDownloadProgress(handler: (payload: DownloadProgressPayload) => void) {
  return listen('download://progress', (event) =>
    handler(event.payload as DownloadProgressPayload),
  );
}

export function listenDownloadStatus(handler: (payload: DownloadStatusPayload) => void) {
  return listen('download://status', (event) =>
    handler(event.payload as DownloadStatusPayload),
  );
}

export function listenDownloadError(handler: (payload: DownloadStatusPayload) => void) {
  return listen('download://error', (event) =>
    handler(event.payload as DownloadStatusPayload),
  );
}
