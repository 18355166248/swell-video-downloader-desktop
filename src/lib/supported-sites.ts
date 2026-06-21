export type SupportedSite = {
  host: string;
  label: string;
};

// Single source of truth for the sites the resolver/downloader can handle. Keep
// in sync with `resolve_source` on the Rust side. More sites will be added here.
export const SUPPORTED_VIDEO_SITES: SupportedSite[] = [
  { host: 'x.com', label: 'X (Twitter)' },
  { host: 'pornhub.com', label: 'Pornhub' },
  { host: 'instagram.com', label: 'Instagram' },
];

export function isSupportedVideoUrl(url: string): boolean {
  let host: string;
  try {
    host = new URL(url).hostname.replace(/^www\./, '');
  } catch {
    return false;
  }
  return SUPPORTED_VIDEO_SITES.some(
    (site) => host === site.host || host.endsWith(`.${site.host}`),
  );
}

export function supportedSitesLabel(): string {
  return SUPPORTED_VIDEO_SITES.map((site) => site.label).join('、');
}
