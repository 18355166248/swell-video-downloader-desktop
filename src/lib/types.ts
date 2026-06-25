export type MediaFormat = {
  id: string;
  label: string;
  ext: string;
  hasAudio: boolean;
  note: string;
  sizeBytes?: number | null;
};

export type ResolveMediaResponse = {
  title: string;
  source: 'x.com' | 'pornhub.com' | 'instagram.com';
  durationText: string;
  recommendation: MediaFormat;
  formats: MediaFormat[];
  thumbnail?: string | null;
};

export type InstagramAuthMode = 'sessionid' | 'cookies_txt';

export type InstagramCollectMode =
  | 'single'
  | 'detail_next'
  | 'profile_recent'
  | 'story_experimental';

export type InstagramCollectItem = {
  url: string;
  kind: 'post' | 'reel' | 'story' | 'unknown';
  sourceLabel: string;
  thumbnailHint?: string | null;
};

export type CollectInstagramTargetsResponse = {
  items: InstagramCollectItem[];
  resolvedCount: number;
  warnings: string[];
  cookieBridgeFilePath?: string | null;
};

export type DiagnosticErrorCategory =
  | 'binary_missing'
  | 'spawn_failed'
  | 'timeout'
  | 'cookie_locked'
  | 'cookie_file_missing'
  | 'proxy_or_network'
  | 'login_or_access_required'
  | 'audience_restricted'
  | 'geo_restricted'
  | 'extractor_changed'
  | 'unknown';

export type DiagnosticCommandPreview = {
  program: string;
  args: string[];
  displayCommand: string;
};

export type MediaDiagnostics = {
  cookieMode: string;
  ytDlpSource: string;
  ffmpegSource: string;
  proxyEnabled: boolean;
  commandPreview: DiagnosticCommandPreview;
  formatsCount: number;
  bestFormatId?: string | null;
  bestHeight?: number | null;
  maxHeight?: number | null;
  bestHasAudio: boolean;
  hasMuxedFormat: boolean;
  hasVideoOnlyFormat: boolean;
  hasAudioOnlyFormat: boolean;
  errorCategory?: DiagnosticErrorCategory | null;
  normalizedMessage?: string | null;
  rawErrorMessage?: string | null;
};

export type DiagnoseMediaResponse = {
  resolved?: ResolveMediaResponse | null;
  diagnostics: MediaDiagnostics;
};

export type DiagnosticComparisonResult = {
  kind:
    | 'same_quality'
    | 'quality_limited'
    | 'none_requires_access'
    | 'inconclusive';
  message: string;
};

export type CookieSource = {
  id: string;
  label: string;
};

export type DependencyStatus = {
  ytDlpOk: boolean;
  ffmpegOk: boolean;
  ytDlpSource: string;
  ffmpegSource: string;
};

export type DownloadDirectorySettings = {
  currentDir: string;
  defaultDir: string;
  isCustom: boolean;
};

export type AppSettings = {
  cookieSource: string;
  cookieFilePath: string;
  instagramSessionId: string;
  instagramCookieFilePath: string;
  instagramCollectMode: InstagramCollectMode;
  instagramCollectCount: string;
  autoDownload: boolean;
  downloadConcurrency: number;
};
