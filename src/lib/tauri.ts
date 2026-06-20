import { invoke } from '@tauri-apps/api/core';
import type { CookieSource, DependencyStatus, ResolveMediaResponse } from './types';

type ResolveMediaWireResponse = {
  title: string;
  source: 'x.com' | 'pornhub.com';
  duration_text: string;
  recommendation: {
    id: string;
    label: string;
    ext: string;
    has_audio: boolean;
    note: string;
  };
  formats: Array<{
    id: string;
    label: string;
    ext: string;
    has_audio: boolean;
    note: string;
  }>;
};

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
    recommendation: {
      id: response.recommendation.id,
      label: response.recommendation.label,
      ext: response.recommendation.ext,
      hasAudio: response.recommendation.has_audio,
      note: response.recommendation.note,
    },
    formats: response.formats.map((format) => ({
      id: format.id,
      label: format.label,
      ext: format.ext,
      hasAudio: format.has_audio,
      note: format.note,
    })),
  };
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
