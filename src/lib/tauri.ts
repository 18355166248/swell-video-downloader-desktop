import { invoke } from '@tauri-apps/api/core';
import { openPath, revealItemInDir } from '@tauri-apps/plugin-opener';
import type {
  AppSettings,
  CollectInstagramTargetsResponse,
  CookieSource,
  DiagnoseMediaResponse,
  DiagnosticErrorCategory,
  DependencyStatus,
  DownloadDirectorySettings,
  InstagramCollectItem,
  InstagramCollectMode,
  ResolveMediaResponse,
} from './types';
import { normalizeDownloadConcurrency } from './settings';

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
  source: 'x.com' | 'pornhub.com' | 'instagram.com';
  duration_text: string;
  recommendation: MediaFormatWire;
  formats: MediaFormatWire[];
  thumbnail?: string | null;
};

type DiagnosticCommandPreviewWire = {
  program: string;
  args: string[];
  display_command: string;
};

type MediaDiagnosticsWire = {
  cookie_mode: string;
  yt_dlp_source: string;
  ffmpeg_source: string;
  proxy_enabled: boolean;
  command_preview: DiagnosticCommandPreviewWire;
  formats_count: number;
  best_format_id?: string | null;
  best_height?: number | null;
  max_height?: number | null;
  best_has_audio: boolean;
  has_muxed_format: boolean;
  has_video_only_format: boolean;
  has_audio_only_format: boolean;
  error_category?: string | null;
  normalized_message?: string | null;
  raw_error_message?: string | null;
};

type DiagnoseMediaWireResponse = {
  resolved?: ResolveMediaWireResponse | null;
  diagnostics: MediaDiagnosticsWire;
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

function mapResolveResponse(response: ResolveMediaWireResponse): ResolveMediaResponse {
  return {
    title: response.title,
    source: response.source,
    durationText: response.duration_text,
    recommendation: mapFormat(response.recommendation),
    formats: response.formats.map(mapFormat),
    thumbnail: response.thumbnail ?? null,
  };
}

export function getTauriErrorMessage(error: unknown, fallback: string): string {
  if (error instanceof Error && error.message.trim()) {
    return error.message;
  }

  if (typeof error === 'string' && error.trim()) {
    return error;
  }

  if (error && typeof error === 'object') {
    const maybeMessage = 'message' in error ? error.message : null;
    if (typeof maybeMessage === 'string' && maybeMessage.trim()) {
      return maybeMessage;
    }

    const maybeError = 'error' in error ? error.error : null;
    if (typeof maybeError === 'string' && maybeError.trim()) {
      return maybeError;
    }
  }

  return fallback;
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
    ...mapResolveResponse(response),
  };
}

export async function diagnoseMedia(
  url: string,
  cookieSource?: string | null,
  cookieFilePath?: string | null,
): Promise<DiagnoseMediaResponse> {
  const response = await invoke<DiagnoseMediaWireResponse>('diagnose_media', {
    url,
    cookieSource,
    cookieFilePath,
  });

  return {
    resolved: response.resolved ? mapResolveResponse(response.resolved) : null,
    diagnostics: {
      cookieMode: response.diagnostics.cookie_mode,
      ytDlpSource: response.diagnostics.yt_dlp_source,
      ffmpegSource: response.diagnostics.ffmpeg_source,
      proxyEnabled: response.diagnostics.proxy_enabled,
      commandPreview: {
        program: response.diagnostics.command_preview.program,
        args: response.diagnostics.command_preview.args,
        displayCommand: response.diagnostics.command_preview.display_command,
      },
      formatsCount: response.diagnostics.formats_count,
      bestFormatId: response.diagnostics.best_format_id ?? null,
      bestHeight: response.diagnostics.best_height ?? null,
      maxHeight: response.diagnostics.max_height ?? null,
      bestHasAudio: response.diagnostics.best_has_audio,
      hasMuxedFormat: response.diagnostics.has_muxed_format,
      hasVideoOnlyFormat: response.diagnostics.has_video_only_format,
      hasAudioOnlyFormat: response.diagnostics.has_audio_only_format,
      errorCategory: (response.diagnostics.error_category as DiagnosticErrorCategory | null) ?? null,
      normalizedMessage: response.diagnostics.normalized_message ?? null,
      rawErrorMessage: response.diagnostics.raw_error_message ?? null,
    },
  };
}

type InstagramCollectItemWire = {
  url: string;
  kind: string;
  source_label: string;
  thumbnail_hint?: string | null;
};

type CollectInstagramTargetsWireResponse = {
  items: InstagramCollectItemWire[];
  resolved_count: number;
  warnings: string[];
  cookie_bridge_file_path?: string | null;
};

export async function collectInstagramTargets(
  url: string,
  mode: InstagramCollectMode,
  count: number,
  sessionid?: string | null,
  cookieFilePath?: string | null,
): Promise<CollectInstagramTargetsResponse> {
  // Nested struct fields are deserialized by serde with their snake_case names,
  // so the request object must use snake_case keys (unlike top-level args).
  const response = await invoke<CollectInstagramTargetsWireResponse>('collect_instagram_targets', {
    request: {
      url,
      mode,
      count,
      sessionid: sessionid ?? null,
      cookie_file_path: cookieFilePath ?? null,
    },
  });

  return {
    items: response.items.map<InstagramCollectItem>((item) => ({
      url: item.url,
      kind: item.kind as InstagramCollectItem['kind'],
      sourceLabel: item.source_label,
      thumbnailHint: item.thumbnail_hint ?? null,
    })),
    resolvedCount: response.resolved_count,
    warnings: response.warnings,
    cookieBridgeFilePath: response.cookie_bridge_file_path ?? null,
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

type AppSettingsWire = {
  cookie_source?: string | null;
  cookie_file_path?: string | null;
  instagram_sessionid?: string | null;
  instagram_cookie_file_path?: string | null;
  instagram_collect_mode?: string | null;
  instagram_collect_count?: string | null;
  auto_download?: boolean | null;
  download_concurrency?: number | null;
};

function mapAppSettings(wire: AppSettingsWire): AppSettings {
  return {
    cookieSource: wire.cookie_source ?? '',
    cookieFilePath: wire.cookie_file_path ?? '',
    instagramSessionId: wire.instagram_sessionid ?? '',
    instagramCookieFilePath: wire.instagram_cookie_file_path ?? '',
    instagramCollectMode: (wire.instagram_collect_mode as InstagramCollectMode) || 'single',
    instagramCollectCount: wire.instagram_collect_count ?? '1',
    autoDownload: wire.auto_download ?? false,
    downloadConcurrency: normalizeDownloadConcurrency(wire.download_concurrency),
  };
}

export async function getAppSettings(): Promise<AppSettings> {
  const wire = await invoke<AppSettingsWire>('get_app_settings');
  return mapAppSettings(wire);
}

export async function saveAppSettings(settings: AppSettings): Promise<AppSettings> {
  // Nested struct fields use serde snake_case names.
  const wire = await invoke<AppSettingsWire>('set_app_settings', {
    settings: {
      cookie_source: settings.cookieSource || null,
      cookie_file_path: settings.cookieFilePath || null,
      instagram_sessionid: settings.instagramSessionId || null,
      instagram_cookie_file_path: settings.instagramCookieFilePath || null,
      instagram_collect_mode: settings.instagramCollectMode || null,
      instagram_collect_count: settings.instagramCollectCount || null,
      auto_download: settings.autoDownload,
      download_concurrency: normalizeDownloadConcurrency(settings.downloadConcurrency),
    },
  });
  return mapAppSettings(wire);
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

export async function cancelDownload(taskId: string) {
  return invoke<void>('cancel_download', { taskId });
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
