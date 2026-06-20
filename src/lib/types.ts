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
  source: 'x.com' | 'pornhub.com';
  durationText: string;
  recommendation: MediaFormat;
  formats: MediaFormat[];
  thumbnail?: string | null;
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
