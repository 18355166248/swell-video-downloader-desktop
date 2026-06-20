export type MediaFormat = {
  id: string;
  label: string;
  ext: string;
  hasAudio: boolean;
  note: string;
};

export type ResolveMediaResponse = {
  title: string;
  source: 'x.com' | 'pornhub.com';
  durationText: string;
  recommendation: MediaFormat;
  formats: MediaFormat[];
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
