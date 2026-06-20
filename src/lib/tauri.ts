import { invoke } from '@tauri-apps/api/core';
import { openPath, revealItemInDir } from '@tauri-apps/plugin-opener';
import type {
  CookieSource,
  DependencyStatus,
  DownloadDirectorySettings,
  ResolveMediaResponse,
} from './types';

type MediaFormatWire = {
  id: string;
  label: string;
  ext: string;
  has_audio: boolean;
  note: string;
  size_bytes?: number | null;
};

type ResolveMediaWireResponse = {
  title: string;
  source: 'x.com' | 'pornhub.com';
  duration_text: string;
  recommendation: MediaFormatWire;
  formats: MediaFormatWire[];
  thumbnail?: string | null;
};

function mapFormat(format: MediaFormatWire) {
  return {
    id: format.id,
    label: format.label,
    ext: format.ext,
    hasAudio: format.has_audio,
    note: format.note,
    sizeBytes: format.size_bytes ?? null,
  };
}

export async function resolveMedia(
  url: string,
  cookieSource?: string | null,
  cookieFilePath?: string | null,
): Promise<ResolveMediaResponse> {
  const response = await invoke<ResolveMediaWireResponse>('resolve_media', {
    url,
    cookieSource,
    cookieFilePath,
  });

  return {
    title: response.title,
    source: response.source,
    durationText: response.duration_text,
    recommendation: mapFormat(response.recommendation),
    formats: response.formats.map(mapFormat),
    thumbnail: response.thumbnail ?? null,
  };
}

export async function generatePreview(url: string, formatId: string): Promise<string> {
  return invoke<string>('generate_preview', { url, formatId });
}

export async function getDownloadDir(): Promise<string> {
  return invoke<string>('get_download_dir');
}

type DownloadDirectorySettingsWire = {
  current_dir: string;
  default_dir: string;
  is_custom: boolean;
};

export async function getDownloadDirectorySettings(): Promise<DownloadDirectorySettings> {
  const response = await invoke<DownloadDirectorySettingsWire>('get_download_dir_settings');
  return {
    currentDir: response.current_dir,
    defaultDir: response.default_dir,
    isCustom: response.is_custom,
  };
}

export async function setDownloadDir(path: string): Promise<string> {
  return invoke<string>('set_download_dir', { path });
}

export async function resetDownloadDir(): Promise<string> {
  return invoke<string>('reset_download_dir');
}

/** Open the download folder, selecting the finished file when its path is known. */
export async function openDownloadLocation(downloadDir: string, filePath?: string | null) {
  if (filePath) {
    return revealItemInDir(filePath);
  }
  return openPath(downloadDir);
}

type CookieSourceWire = {
  id: string;
  label: string;
};

type DependencyStatusWire = {
  yt_dlp_ok: boolean;
  ffmpeg_ok: boolean;
  yt_dlp_source: string;
  ffmpeg_source: string;
};

export async function startDownload(
  url: string,
  formatId?: string | null,
  title?: string | null,
  cookieSource?: string | null,
  cookieFilePath?: string | null,
) {
  return invoke<string>('start_download', { url, formatId, title, cookieSource, cookieFilePath });
}

export async function listCookieSources(): Promise<CookieSource[]> {
  const response = await invoke<CookieSourceWire[]>('list_cookie_sources');
  return response.map((item) => ({
    id: item.id,
    label: item.label,
  }));
}

export async function checkDependencies(): Promise<DependencyStatus> {
  const response = await invoke<DependencyStatusWire>('check_dependencies');
  return {
    ytDlpOk: response.yt_dlp_ok,
    ffmpegOk: response.ffmpeg_ok,
    ytDlpSource: response.yt_dlp_source,
    ffmpegSource: response.ffmpeg_source,
  };
}
