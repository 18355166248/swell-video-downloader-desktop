import { Button, InlineAlert, Text } from '@react-spectrum/s2';
import { useEffect, useRef, useState, type KeyboardEvent as ReactKeyboardEvent } from 'react';
import { TitleBar } from './components/TitleBar';
import { ToastStack, type ToastItem } from './components/ToastStack';
import { DownloadsTable, type DownloadRow } from './features/downloads/DownloadsTable';
import { DiagnosticPanel } from './features/resolve/DiagnosticPanel';
import { ResolveBoard, type ResolveItem } from './features/resolve/ResolveBoard';
import { ResolvePanel } from './features/resolve/ResolvePanel';
import { SettingsDrawer } from './features/settings/SettingsDrawer';
import {
  listenDownloadError,
  listenDownloadProgress,
  listenDownloadStatus,
} from './lib/download-events';
import {
  cancelDownload,
  checkDependencies,
  collectInstagramTargets,
  diagnoseMedia,
  generatePreview,
  getAppSettings,
  getDownloadDirectorySettings,
  getTauriErrorMessage,
  listCookieSources,
  openDownloadLocation,
  resetDownloadDir,
  resolveMedia,
  saveAppSettings,
  setDownloadDir,
  startDownload,
} from './lib/tauri';
import { isSupportedVideoUrl, supportedSitesLabel } from './lib/supported-sites';
import type {
  AppSettings,
  CookieSource,
  DiagnoseMediaResponse,
  DiagnosticComparisonResult,
  DependencyStatus,
  DownloadDirectorySettings,
  InstagramCollectMode,
  MediaFormat,
  ResolveMediaResponse,
} from './lib/types';

type DownloadBucketState = {
  current: DownloadRow[];
  history: DownloadRow[];
};

type DownloadTab = 'current' | 'history';

const DOWNLOAD_HISTORY_STORAGE_KEY = 'swell.downloadHistory.v1';
const TOAST_LIMIT = 4;

const HERO_STEPS = [
  { id: 1, label: '解析地址' },
  { id: 2, label: '选择版本' },
  { id: 3, label: '下载队列' },
  { id: 4, label: '设置' },
] as const;

function loadStoredHistory(): DownloadRow[] {
  if (typeof window === 'undefined') {
    return [];
  }

  try {
    const raw = window.localStorage.getItem(DOWNLOAD_HISTORY_STORAGE_KEY);
    if (!raw) {
      return [];
    }

    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? (parsed as DownloadRow[]) : [];
  } catch {
    return [];
  }
}

function mergeRowsById(primary: DownloadRow[], secondary: DownloadRow[]): DownloadRow[] {
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

const ACTIVE_STATUSES = ['queued', 'downloading', 'postprocessing', 'canceling'];

function isActiveStatus(status: string): boolean {
  return ACTIVE_STATUSES.includes(status);
}

// Move only finished (completed/failed/canceled) rows into history; downloads
// still in flight stay in the current queue so a new resolve never kills them.
function archiveFinishedRows(state: DownloadBucketState): DownloadBucketState {
  const finished = state.current.filter((row) => !isActiveStatus(row.status));
  if (finished.length === 0) {
    return state;
  }

  return {
    current: state.current.filter((row) => isActiveStatus(row.status)),
    history: mergeRowsById(finished, state.history),
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

  if (state.current.some((row) => row.id === taskId)) {
    return {
      ...state,
      current: updateList(state.current),
    };
  }

  if (state.history.some((row) => row.id === taskId)) {
    return {
      ...state,
      history: updateList(state.history),
    };
  }

  return {
    ...state,
    history: [fallbackRow, ...state.history],
  };
}

function createFallbackRow(taskId: string, title: string, status: string): DownloadRow {
  return {
    id: taskId,
    title,
    sessionLabel: '历史任务',
    status,
    progress: status === 'completed' ? '100%' : '0%',
  };
}

function deriveSessionLabel(url: string, result?: ResolveMediaResponse | null): string {
  if (result?.source) {
    return result.source;
  }

  try {
    return new URL(url).hostname.replace(/^www\./, '');
  } catch {
    return '本次解析';
  }
}

function cleanFormatLabel(label: string): string {
  const cleaned = label.trim().replace(/^下载\s*/u, '').trim();
  return cleaned || label.trim();
}

function buildDownloadTitle(baseTitle: string, format: MediaFormat): string {
  const descriptor = cleanFormatLabel(format.label);
  return baseTitle.includes(descriptor) ? baseTitle : `${baseTitle} - ${descriptor}`;
}

function parseBatchUrls(value: string): string[] {
  return value
    .split(/\r?\n/u)
    .map((item) => item.trim())
    .filter((item) => item.length > 0);
}

// Front-end gate so garbage input never reaches the resolver. Accepts bare hosts
// like "x.com/..." by assuming https, and returns the normalized URL (or null).
function normalizeVideoUrl(raw: string): string | null {
  const value = raw.trim();
  if (!value) {
    return null;
  }

  const hasScheme = /^[a-z][a-z0-9+.-]*:\/\//iu.test(value);
  let parsed: URL;
  try {
    parsed = new URL(hasScheme ? value : `https://${value}`);
  } catch {
    return null;
  }

  if (parsed.protocol !== 'http:' && parsed.protocol !== 'https:') {
    return null;
  }
  // Reject hosts without a dotted domain (e.g. "asdf", "localhost typo").
  if (!/^[a-z0-9.-]+\.[a-z]{2,}$/iu.test(parsed.hostname)) {
    return null;
  }

  return parsed.toString();
}

function isInstagramUrl(url: string): boolean {
  try {
    return new URL(url).hostname.includes('instagram.com');
  } catch {
    return false;
  }
}

function summarizeTitle(title: string, maxLength = 36): string {
  if (title.length <= maxLength) {
    return title;
  }
  return `${title.slice(0, maxLength - 1)}…`;
}

function compareDiagnostics(
  previous: DiagnoseMediaResponse | null,
  current: DiagnoseMediaResponse | null,
): DiagnosticComparisonResult | null {
  if (!previous || !current) {
    return null;
  }

  const pair = [previous, current];
  const noneResult = pair.find((item) => item.diagnostics.cookieMode === 'none');
  const accessResult = pair.find((item) => item.diagnostics.cookieMode !== 'none');

  if (!noneResult || !accessResult) {
    return {
      kind: 'inconclusive',
      message: '两次结果差异不明显或都失败',
    };
  }

  if (noneResult.resolved && accessResult.resolved) {
    const noneMax = noneResult.diagnostics.maxHeight ?? 0;
    const accessMax = accessResult.diagnostics.maxHeight ?? 0;

    if (noneMax > 0 && noneMax >= accessMax) {
      return {
        kind: 'same_quality',
        message: '无 Cookie 已可拿到最高画质',
      };
    }

    if (noneMax > 0 && accessMax > noneMax) {
      return {
        kind: 'quality_limited',
        message: '无 Cookie 可解析，但画质低于带 Cookie',
      };
    }
  }

  if (!noneResult.resolved && accessResult.resolved) {
    return {
      kind: 'none_requires_access',
      message: '无 Cookie 无法解析，带 Cookie 可解析',
    };
  }

  return {
    kind: 'inconclusive',
    message: '两次结果差异不明显或都失败',
  };
}


export default function App() {
  const [urls, setUrls] = useState('');
  const [resolvedItems, setResolvedItems] = useState<ResolveItem[]>([]);
  const [expandedUrl, setExpandedUrl] = useState<string | null>(null);
  const [error, setError] = useState('');
  const [isResolving, setIsResolving] = useState(false);
  const [downloadState, setDownloadState] = useState<DownloadBucketState>({
    current: [],
    history: loadStoredHistory(),
  });
  const [downloadTab, setDownloadTab] = useState<DownloadTab>('current');
  const [downloadDir, setDownloadDirState] = useState('');
  const [downloadDirectorySettings, setDownloadDirectorySettings] =
    useState<DownloadDirectorySettings | null>(null);
  const [downloadDirectoryDraft, setDownloadDirectoryDraft] = useState('');
  const [isSavingDownloadDirectory, setIsSavingDownloadDirectory] = useState(false);
  const [cookieSources, setCookieSources] = useState<CookieSource[]>([]);
  const [selectedCookieSource, setSelectedCookieSource] = useState('chrome');
  const [cookieFilePath, setCookieFilePath] = useState('');
  const [instagramSessionId, setInstagramSessionId] = useState('');
  const [instagramCookieFilePath, setInstagramCookieFilePath] = useState('');
  const [instagramCollectMode, setInstagramCollectMode] =
    useState<InstagramCollectMode>('single');
  const [instagramCollectCount, setInstagramCollectCount] = useState('1');
  // Ephemeral cookies.txt exported by the Instagram collector for the active run.
  // Kept separate from the user-entered cookieFilePath so it is never persisted.
  const [instagramBridgeCookiePath, setInstagramBridgeCookiePath] = useState('');
  const [dependencyStatus, setDependencyStatus] = useState<DependencyStatus | null>(null);
  const [isDiagnosing, setIsDiagnosing] = useState(false);
  const [diagnosticResult, setDiagnosticResult] = useState<DiagnoseMediaResponse | null>(null);
  const [diagnosticUrl, setDiagnosticUrl] = useState('');
  const [previousDiagnosticResult, setPreviousDiagnosticResult] =
    useState<DiagnoseMediaResponse | null>(null);
  const [downloadingIds, setDownloadingIds] = useState<Set<string>>(new Set());
  const [toasts, setToasts] = useState<ToastItem[]>([]);
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);
  const notifiedDownloadIds = useRef<Set<string>>(new Set());
  // Task ids the user removed from the list — late progress/status events for
  // these are ignored so a deleted row never re-appears.
  const dismissedDownloadIds = useRef<Set<string>>(new Set());
  // Monotonic token identifying the active resolve run. Bumping it cancels the
  // in-flight run: stale awaits see a mismatch and bail without touching state.
  const resolveSessionRef = useRef(0);

  function dismissToast(id: string) {
    setToasts((current) => current.filter((toast) => toast.id !== id));
  }

  function pushToast(
    message: string,
    variant: ToastItem['variant'] = 'info',
    durationMs?: number,
  ) {
    setToasts((current) => [
      ...current.slice(-(TOAST_LIMIT - 1)),
      {
        id: `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
        message,
        variant,
        durationMs,
      },
    ]);
  }

  function reportError(message: string) {
    setError(message);
    pushToast(message, 'error');
  }

  // Persist the current settings snapshot (with the just-changed field applied)
  // to the device-level config file so values survive restarts and reinstalls.
  // Only user-edited fields flow through here — the ephemeral Instagram cookie
  // bridge is never persisted.
  function persistSettings(partial: Partial<AppSettings>) {
    const snapshot: AppSettings = {
      cookieSource: selectedCookieSource,
      cookieFilePath,
      instagramSessionId,
      instagramCookieFilePath,
      instagramCollectMode,
      instagramCollectCount,
      ...partial,
    };
    void saveAppSettings(snapshot).catch((err) => {
      reportError(getTauriErrorMessage(err, '保存设置失败'));
    });
  }

  function handleCookieSourceChange(value: string) {
    setSelectedCookieSource(value);
    persistSettings({ cookieSource: value });
  }

  function handleCookieFilePathChange(value: string) {
    setCookieFilePath(value);
    persistSettings({ cookieFilePath: value });
  }

  function handleInstagramSessionIdChange(value: string) {
    setInstagramSessionId(value);
    persistSettings({ instagramSessionId: value });
  }

  function handleInstagramCookieFilePathChange(value: string) {
    setInstagramCookieFilePath(value);
    persistSettings({ instagramCookieFilePath: value });
  }

  function handleInstagramCollectModeChange(value: AppSettings['instagramCollectMode']) {
    setInstagramCollectMode(value);
    persistSettings({ instagramCollectMode: value });
  }

  function handleInstagramCollectCountChange(value: string) {
    setInstagramCollectCount(value);
    persistSettings({ instagramCollectCount: value });
  }

  useEffect(() => {
    async function loadSettingsData() {
      const [sources, dependencies, dirSettings, appSettings] = await Promise.all([
        listCookieSources(),
        checkDependencies(),
        getDownloadDirectorySettings(),
        getAppSettings(),
      ]);
      setCookieSources(sources);
      // Restore the saved cookie source if it still exists; otherwise default to
      // the first available source.
      const savedSource = appSettings.cookieSource;
      const validSavedSource =
        savedSource && sources.some((item) => item.id === savedSource) ? savedSource : '';
      if (validSavedSource) {
        setSelectedCookieSource(validSavedSource);
      } else if (sources.length > 0) {
        setSelectedCookieSource(sources[0].id);
      }
      setCookieFilePath(appSettings.cookieFilePath);
      setInstagramSessionId(appSettings.instagramSessionId);
      setInstagramCookieFilePath(appSettings.instagramCookieFilePath);
      setInstagramCollectMode(appSettings.instagramCollectMode);
      setInstagramCollectCount(appSettings.instagramCollectCount);
      setDependencyStatus(dependencies);
      setDownloadDirectorySettings(dirSettings);
      setDownloadDirState(dirSettings.currentDir);
      setDownloadDirectoryDraft(dirSettings.currentDir);
    }

    loadSettingsData().catch((err) => {
      reportError(getTauriErrorMessage(err, '初始化设置数据失败'));
    });
  }, []);

  useEffect(() => {
    if (typeof window === 'undefined') {
      return;
    }
    window.localStorage.setItem(
      DOWNLOAD_HISTORY_STORAGE_KEY,
      JSON.stringify(downloadState.history),
    );
  }, [downloadState.history]);

  useEffect(() => {
    const unlisteners: Array<() => void> = [];

    Promise.all([
      listenDownloadProgress((payload) => {
        if (dismissedDownloadIds.current.has(payload.task_id)) {
          return;
        }
        setDownloadState((current) =>
          updateRowCollections(
            current,
            payload.task_id,
            (row) => ({
              ...row,
              progress: payload.percent,
              speed: payload.speed,
            }),
            {
              ...createFallbackRow(payload.task_id, '历史任务', 'downloading'),
              progress: payload.percent,
              speed: payload.speed,
            },
          ),
        );
      }),
      listenDownloadStatus((payload) => {
        if (dismissedDownloadIds.current.has(payload.task_id)) {
          return;
        }
        if (
          (payload.status === 'completed' || payload.status === 'failed') &&
          !notifiedDownloadIds.current.has(payload.task_id)
        ) {
          notifiedDownloadIds.current.add(payload.task_id);

          if (payload.status === 'completed') {
            pushToast(`下载完成：${summarizeTitle(payload.title)}`, 'success');
          } else {
            pushToast(
              payload.message
                ? `下载失败：${payload.message}`
                : `下载失败：${summarizeTitle(payload.title)}`,
              'error',
            );
          }
        }

        setDownloadState((current) =>
          updateRowCollections(
            current,
            payload.task_id,
            (row) => ({
              ...row,
              title: payload.title,
              status: payload.status,
              progress:
                payload.status === 'completed'
                  ? '100%'
                  : payload.status === 'failed'
                    ? row.progress || '0%'
                    : row.progress,
              outputPath: payload.output_path ?? row.outputPath,
            }),
            {
              ...createFallbackRow(payload.task_id, payload.title, payload.status),
              outputPath: payload.output_path ?? null,
            },
          ),
        );
      }),
      listenDownloadError((payload) => {
        if (payload.task_id && dismissedDownloadIds.current.has(payload.task_id)) {
          return;
        }
        if (payload.message) {
          if (payload.task_id && notifiedDownloadIds.current.has(payload.task_id)) {
            return;
          }
          if (payload.task_id) {
            notifiedDownloadIds.current.add(payload.task_id);
          }
          reportError(payload.message);
        }
      }),
    ])
      .then((handlers) => {
        handlers.forEach((handler) => unlisteners.push(handler));
      })
      .catch((err) => {
        reportError(getTauriErrorMessage(err, '注册下载事件失败'));
      });

    return () => {
      unlisteners.forEach((dispose) => dispose());
    };
  }, []);

  function patchResolvedItem(itemUrl: string, patch: Partial<ResolveItem>) {
    setResolvedItems((items) =>
      items.map((item) => (item.url === itemUrl ? { ...item, ...patch } : item)),
    );
  }

  // Format ids are only unique within one video, so downloads are tracked under
  // a `url::formatId` key. Each card gets the bare ids scoped to its own url.
  function scopedDownloadingIds(itemUrl: string): Set<string> {
    const prefix = `${itemUrl}::`;
    const scoped = new Set<string>();
    for (const key of downloadingIds) {
      if (key.startsWith(prefix)) {
        scoped.add(key.slice(prefix.length));
      }
    }
    return scoped;
  }

  async function loadPreview(itemUrl: string, result: ResolveMediaResponse) {
    // yt-dlp paths come with a poster URL already; only the ssstwitter fallback
    // needs an ffmpeg-generated frame. Use the smallest variant for speed.
    if (result.thumbnail) {
      patchResolvedItem(itemUrl, { thumbnail: result.thumbnail });
      return;
    }

    const previewFormat = result.formats[result.formats.length - 1] ?? result.recommendation;
    if (!previewFormat) {
      return;
    }

    patchResolvedItem(itemUrl, { isPreviewLoading: true });
    try {
      const dataUrl = await generatePreview(itemUrl, previewFormat.id);
      patchResolvedItem(itemUrl, { thumbnail: dataUrl, isPreviewLoading: false });
    } catch {
      patchResolvedItem(itemUrl, { thumbnail: null, isPreviewLoading: false });
    }
  }

  async function handleDiagnose() {
    if (isDiagnosing) {
      return;
    }

    const lines = parseBatchUrls(urls);
    if (lines.length === 0) {
      reportError('请先输入要诊断的视频地址。');
      return;
    }
    if (lines.length !== 1) {
      reportError('诊断解析一次只支持 1 条链接，请先保留一个地址。');
      return;
    }

    const targetUrl = normalizeVideoUrl(lines[0]);
    if (!targetUrl) {
      reportError('请输入有效的视频地址后再诊断。');
      return;
    }

    setError('');
    setIsDiagnosing(true);
    try {
      const result = await diagnoseMedia(targetUrl, selectedCookieSource, cookieFilePath);
      setPreviousDiagnosticResult(diagnosticResult);
      setDiagnosticResult(result);
      setDiagnosticUrl(targetUrl);
      if (result.resolved) {
        pushToast(`诊断完成：${summarizeTitle(result.resolved.title)}`, 'success');
      } else {
        pushToast('诊断完成：未解析成功，可查看诊断结果继续排查。', 'info');
      }
    } catch (err) {
      reportError(getTauriErrorMessage(err, '诊断解析失败'));
    } finally {
      setIsDiagnosing(false);
    }
  }

  async function handleCopyDiagnosticCommand() {
    if (!diagnosticResult) {
      return;
    }

    try {
      await navigator.clipboard.writeText(diagnosticResult.diagnostics.commandPreview.displayCommand);
      pushToast('CLI 复现命令已复制。', 'success');
    } catch {
      reportError('复制 CLI 命令失败，请检查剪贴板权限。');
    }
  }

  function handleApplyDiagnosticResolved() {
    if (!diagnosticResult?.resolved) {
      return;
    }

    const item = {
      url: diagnosticUrl,
      status: 'ready' as const,
      resolved: diagnosticResult.resolved,
      thumbnail: diagnosticResult.resolved.thumbnail ?? null,
      isPreviewLoading: false,
    };

    setResolvedItems([item]);
    setExpandedUrl(item.url);
    pushToast('已将诊断结果填充到解析列表。', 'success');
  }

  function handleSubmit() {
    // Pressing again while resolving cancels the run.
    if (isResolving) {
      resolveSessionRef.current += 1;
      setIsResolving(false);
      // Drop the rows that never finished resolving; keep ready/failed ones.
      setResolvedItems((items) => items.filter((item) => item.status !== 'loading'));
      pushToast('已取消解析。', 'info');
      return;
    }

    const lines = parseBatchUrls(urls);
    if (lines.length === 0) {
      reportError('请先输入视频地址。');
      return;
    }

    const valid: string[] = [];
    const invalid: string[] = [];
    for (const line of lines) {
      const normalized = normalizeVideoUrl(line);
      if (normalized) {
        valid.push(normalized);
      } else {
        invalid.push(line);
      }
    }

    // Any bad line blocks the whole run so the user fixes it before resolving.
    if (invalid.length > 0) {
      reportError(`以下不是有效的视频地址，请检查后重试：${invalid.join('；')}`);
      return;
    }

    // Valid URLs that point at sites we don't support yet get a friendly hint
    // listing what is currently supported, instead of a backend error later.
    const unsupported = valid.filter((value) => !isSupportedVideoUrl(value));
    if (unsupported.length > 0) {
      reportError(
        `暂不支持以下网站的视频：${unsupported.join('；')}。目前支持：${supportedSitesLabel()}。`,
      );
      return;
    }

    const unique = Array.from(new Set(valid));

    // Instagram entries (a single post/reel, or a profile link) go through the
    // Playwright collector first; the canonical URLs it returns then flow into
    // the existing resolve queue.
    if (unique.length === 1 && isInstagramUrl(unique[0])) {
      void handleResolveInstagram(unique[0]);
      return;
    }

    void handleResolveAll(unique);
  }

  async function handleResolveInstagram(entryUrl: string) {
    const count = Math.max(1, Number.parseInt(instagramCollectCount || '1', 10) || 1);
    setIsResolving(true);
    setError('');
    try {
      const collected = await collectInstagramTargets(
        entryUrl,
        instagramCollectMode,
        count,
        instagramSessionId || null,
        instagramCookieFilePath || null,
      );

      collected.warnings.forEach((warning) => pushToast(warning, 'info'));

      const urls = Array.from(new Set(collected.items.map((item) => item.url)));
      if (urls.length === 0) {
        reportError('Instagram 采集未返回任何内容，请检查登录态或链接后重试。');
        setIsResolving(false);
        return;
      }

      // Reuse the collector's exported cookies for resolve + download via a
      // dedicated state, so the persisted cookieFilePath is left untouched.
      const bridgePath = collected.cookieBridgeFilePath || '';
      setInstagramBridgeCookiePath(bridgePath);

      pushToast(`已采集 ${urls.length} 条 Instagram 内容。`, 'success');
      await handleResolveAll(urls, bridgePath || undefined);
    } catch (err) {
      reportError(getTauriErrorMessage(err, 'Instagram 采集失败'));
      setIsResolving(false);
    }
  }

  // Resolve one or many links, then let the user pick a quality per video.
  // Cards stream in as each link resolves so a slow link can't block the rest.
  async function handleResolveAll(targetUrls: string[], cookieFilePathOverride?: string) {
    const session = (resolveSessionRef.current += 1);
    const isActive = () => resolveSessionRef.current === session;
    // A non-Instagram run clears any stale collector bridge so its cookies don't
    // leak into x.com / pornhub downloads.
    if (cookieFilePathOverride === undefined) {
      setInstagramBridgeCookiePath('');
    }
    // The Instagram collector exports a fresh cookies.txt; use it for this run
    // directly instead of waiting for state to settle.
    const effectiveCookieFilePath = cookieFilePathOverride ?? cookieFilePath;

    setIsResolving(true);
    setError('');
    setExpandedUrl(null);
    setDownloadingIds(new Set());
    setDownloadTab('current');
    // Seed a placeholder row per link so the whole batch shows up immediately as
    // "解析中", then each row flips to ready/failed as its resolve settles.
    setResolvedItems(
      targetUrls.map((targetUrl) => ({
        url: targetUrl,
        status: 'loading',
        resolved: null,
        thumbnail: null,
        isPreviewLoading: false,
      })),
    );
    // Keep in-flight downloads running; only sweep finished ones into history.
    setDownloadState((current) => archiveFinishedRows(current));

    let successCount = 0;
    let lastFormatCount = 0;
    const failures: string[] = [];

    for (const targetUrl of targetUrls) {
      if (!isActive()) {
        return;
      }
      try {
        const result = await resolveMedia(targetUrl, selectedCookieSource, effectiveCookieFilePath);
        if (!isActive()) {
          return;
        }
        patchResolvedItem(targetUrl, { status: 'ready', resolved: result });
        // Auto-expand the first video that becomes ready.
        setExpandedUrl((current) => current ?? targetUrl);
        successCount += 1;
        lastFormatCount = result.formats.length;
        void loadPreview(targetUrl, result);
      } catch (err) {
        if (!isActive()) {
          return;
        }
        const message = getTauriErrorMessage(err, '解析失败');
        patchResolvedItem(targetUrl, { status: 'failed', error: message });
        failures.push(`${targetUrl}：${message}`);
      }
    }

    if (!isActive()) {
      return;
    }

    if (successCount > 0) {
      pushToast(
        successCount > 1
          ? `解析完成：${successCount} 个视频，逐个选择清晰度后下载。`
          : `解析成功：发现 ${lastFormatCount} 个版本，选择清晰度后下载。`,
        'success',
      );
    }
    if (failures.length > 0) {
      reportError(`以下链接解析失败：${failures.join('；')}`);
    }
    setIsResolving(false);
  }

  async function handleDownload(item: ResolveItem, format: MediaFormat) {
    if (!item.resolved) {
      return;
    }
    const result = await enqueueDownload(
      item.url,
      item.resolved,
      format,
      deriveSessionLabel(item.url, item.resolved),
      true,
    );
    if (result.ok) {
      patchResolvedItem(item.url, {
        status: 'selected',
        selectedLabel: cleanFormatLabel(format.label),
      });
    }
  }

  async function enqueueDownload(
    targetUrl: string,
    targetResolved: ResolveMediaResponse,
    format: MediaFormat,
    sessionLabel: string,
    notifyStart: boolean,
  ): Promise<{ ok: true } | { ok: false; message: string }> {
    const downloadKey = `${targetUrl}::${format.id}`;
    setError('');
    setDownloadingIds((current) => new Set(current).add(downloadKey));
    try {
      const taskTitle = buildDownloadTitle(targetResolved.title, format);
      const taskId = await startDownload(
        targetUrl,
        format.id ?? null,
        taskTitle,
        selectedCookieSource,
        instagramBridgeCookiePath || cookieFilePath,
      );
      setDownloadState((current) => ({
        ...current,
        current: [
          {
            id: taskId,
            title: taskTitle,
            sessionLabel,
            status: 'queued',
            progress: '0%',
          },
          ...current.current,
        ],
      }));
      if (notifyStart) {
        pushToast(`已开始下载：${summarizeTitle(taskTitle)}`, 'success');
      }
      return { ok: true };
    } catch (err) {
      const message = getTauriErrorMessage(err, '创建下载任务失败');
      reportError(message);
      return { ok: false, message };
    } finally {
      setDownloadingIds((current) => {
        const next = new Set(current);
        next.delete(downloadKey);
        return next;
      });
    }
  }

  async function handleCancelDownload(row: DownloadRow) {
    try {
      await cancelDownload(row.id);
      pushToast(`正在取消：${summarizeTitle(row.title)}`, 'info');
    } catch (err) {
      reportError(getTauriErrorMessage(err, '取消下载失败'));
    }
  }

  function handleDeleteHistoryRow(row: DownloadRow) {
    setDownloadState((current) => ({
      ...current,
      history: current.history.filter((item) => item.id !== row.id),
    }));
    pushToast(`已删除历史记录：${summarizeTitle(row.title)}`, 'info');
  }

  // Remove a row from the current queue. If it is still downloading, stop the
  // backend task first; ignore its later events so it can't reappear.
  async function handleRemoveCurrentRow(row: DownloadRow) {
    if (isActiveStatus(row.status)) {
      try {
        await cancelDownload(row.id);
      } catch {
        // Task may already have ended; removing the row is still fine.
      }
    }
    dismissedDownloadIds.current.add(row.id);
    notifiedDownloadIds.current.add(row.id);
    setDownloadState((current) => ({
      ...current,
      current: current.current.filter((item) => item.id !== row.id),
    }));
    pushToast(`已移除：${summarizeTitle(row.title)}`, 'info');
  }

  async function handleOpenLocation(row: DownloadRow) {
    try {
      await openDownloadLocation(downloadDir, row.outputPath);
    } catch (err) {
      reportError(getTauriErrorMessage(err, '打开文件夹失败'));
    }
  }

  async function handleOpenDownloadDir() {
    if (!downloadDir) {
      return;
    }
    try {
      await openDownloadLocation(downloadDir);
    } catch (err) {
      reportError(getTauriErrorMessage(err, '打开下载目录失败'));
    }
  }

  async function handleSaveDownloadDirectory() {
    setError('');
    setIsSavingDownloadDirectory(true);
    try {
      const nextDir = await setDownloadDir(downloadDirectoryDraft);
      setDownloadDirState(nextDir);
      setDownloadDirectoryDraft(nextDir);
      setDownloadDirectorySettings((current) => ({
        currentDir: nextDir,
        defaultDir: current?.defaultDir ?? nextDir,
        isCustom: true,
      }));
      pushToast('下载目录已更新。', 'success');
    } catch (err) {
      reportError(getTauriErrorMessage(err, '保存下载目录失败'));
    } finally {
      setIsSavingDownloadDirectory(false);
    }
  }

  async function handleResetDownloadDirectory() {
    setError('');
    setIsSavingDownloadDirectory(true);
    try {
      const nextDir = await resetDownloadDir();
      setDownloadDirState(nextDir);
      setDownloadDirectoryDraft(nextDir);
      setDownloadDirectorySettings((current) => ({
        currentDir: nextDir,
        defaultDir: current?.defaultDir ?? nextDir,
        isCustom: false,
      }));
      pushToast('已恢复默认下载目录。', 'success');
    } catch (err) {
      reportError(getTauriErrorMessage(err, '恢复默认下载目录失败'));
    } finally {
      setIsSavingDownloadDirectory(false);
    }
  }

  const activeStep = resolvedItems.length > 0
    ? 2
    : downloadState.current.length > 0
      ? 3
      : 1;
  const diagnosticComparison = compareDiagnostics(previousDiagnosticResult, diagnosticResult);

  return (
    <>
      <TitleBar />
      <ToastStack toasts={toasts} onDismiss={dismissToast} />

      <div className="app-scroll">
        <main className="app-shell">
          <div className="flow">
        <section className="flow-step intro-step" data-step="01" aria-label="解析">
          <ResolvePanel
            urls={urls}
            urlCount={parseBatchUrls(urls).length}
            isResolving={isResolving}
            isDiagnosing={isDiagnosing}
            onUrlsChange={setUrls}
            onSubmit={handleSubmit}
            onDiagnose={() => void handleDiagnose()}
          />
          <ol className="flow-progress" aria-label="操作流程">
            {HERO_STEPS.map((step) => {
              const state =
                step.id < activeStep ? 'is-done' : step.id === activeStep ? 'is-active' : '';
              const isSettingsStep = step.id === 4;
              return (
                <li
                  key={step.id}
                  className={`flow-progress-step ${state}${isSettingsStep ? ' is-clickable' : ''}`.trim()}
                  {...(isSettingsStep
                    ? {
                        role: 'button' as const,
                        tabIndex: 0,
                        onClick: () => setIsSettingsOpen(true),
                        onKeyDown: (event: ReactKeyboardEvent) => {
                          if (event.key === 'Enter' || event.key === ' ') {
                            event.preventDefault();
                            setIsSettingsOpen(true);
                          }
                        },
                      }
                    : {})}
                >
                  <span className="num">{step.id}</span>
                  <span className="lbl">{step.label}</span>
                </li>
              );
            })}
          </ol>
          {error ? <InlineAlert variant="negative">{error}</InlineAlert> : null}
        </section>

        {diagnosticResult ? (
          <section className="flow-step" data-step="02" aria-label="诊断结果">
            <DiagnosticPanel
              result={diagnosticResult}
              previousResult={previousDiagnosticResult}
              comparison={diagnosticComparison}
              isDiagnosing={isDiagnosing}
              onCopyCommand={() => void handleCopyDiagnosticCommand()}
              onApplyResolved={handleApplyDiagnosticResolved}
            />
          </section>
        ) : null}

        {resolvedItems.length > 0 ? (
          <section className="flow-step" data-step="02" aria-label="选择版本">
            {resolvedItems.length > 1 ? (
              <Text UNSAFE_className="section-kicker">
                共 {resolvedItems.length} 个视频 · 展开任意一个选择清晰度后下载
              </Text>
            ) : null}
            <ResolveBoard
              items={resolvedItems}
              openUrl={expandedUrl}
              downloadingIdsFor={scopedDownloadingIds}
              onOpenChange={setExpandedUrl}
              onDownload={(item, format) => void handleDownload(item, format)}
            />
          </section>
        ) : null}

        <section className="flow-step" data-step="03" aria-label="下载队列">
          <div className="section-head">
            <Text UNSAFE_className="section-kicker">下载队列</Text>
            {downloadDir ? (
              <Button variant="secondary" onPress={handleOpenDownloadDir}>
                打开下载目录
              </Button>
            ) : null}
          </div>
          {downloadDir ? (
            <div className="download-dir-bar">
              <Text UNSAFE_className="download-dir-path">{downloadDir}</Text>
            </div>
          ) : null}

          <div className="download-tabs" role="tablist" aria-label="下载视图切换">
            <button
              type="button"
              role="tab"
              aria-selected={downloadTab === 'current'}
              className={`download-tab${downloadTab === 'current' ? ' is-active' : ''}`}
              onClick={() => setDownloadTab('current')}
            >
              当前下载
            </button>
            <button
              type="button"
              role="tab"
              aria-selected={downloadTab === 'history'}
              className={`download-tab${downloadTab === 'history' ? ' is-active' : ''}`}
              onClick={() => setDownloadTab('history')}
            >
              历史记录
            </button>
          </div>

          {downloadTab === 'current' ? (
            <DownloadsTable
              mode="current"
              ariaLabel="当前下载队列"
              emptyText="解析视频并选择清晰度后，下载任务会出现在这里。"
              rows={downloadState.current}
              onOpenLocation={handleOpenLocation}
              onCancel={handleCancelDownload}
              onDelete={handleRemoveCurrentRow}
            />
          ) : (
            <DownloadsTable
              mode="history"
              ariaLabel="下载历史"
              emptyText="还没有历史记录。完成或失败过的任务会出现在这里。"
              rows={downloadState.history}
              onOpenLocation={handleOpenLocation}
              onDelete={handleDeleteHistoryRow}
            />
          )}
        </section>

          </div>
        </main>
      </div>

      <button
        type="button"
        className="settings-fab"
        aria-label="打开设置"
        onClick={() => setIsSettingsOpen(true)}
      >
        <svg width="20" height="20" viewBox="0 0 24 24" aria-hidden="true">
          <path
            fill="currentColor"
            d="M12 8a4 4 0 1 0 0 8 4 4 0 0 0 0-8m0 2a2 2 0 1 1 0 4 2 2 0 0 1 0-4m-1.4-7l-.4 2.2q-.7.25-1.3.65L6.8 7.2 4.8 8.8l1.1 1.9q-.15.65-.15 1.3t.15 1.3L4.8 15.2l2 1.6 2.1-.85q.6.4 1.3.65l.4 2.2h2.8l.4-2.2q.7-.25 1.3-.65l2.1.85 2-1.6-1.1-1.9q.15-.65.15-1.3t-.15-1.3l1.1-1.9-2-1.6-2.1.85q-.6-.4-1.3-.65L13.4 3z"
          />
        </svg>
        <span>设置</span>
      </button>

      <SettingsDrawer
        open={isSettingsOpen}
        onClose={() => setIsSettingsOpen(false)}
        cookieSources={cookieSources}
        selectedCookieSource={selectedCookieSource}
        cookieFilePath={cookieFilePath}
        instagramSessionId={instagramSessionId}
        instagramCookieFilePath={instagramCookieFilePath}
        instagramCollectMode={instagramCollectMode}
        instagramCollectCount={instagramCollectCount}
        dependencyStatus={dependencyStatus}
        downloadDirectory={downloadDirectorySettings}
        downloadDirectoryDraft={downloadDirectoryDraft}
        isSavingDownloadDirectory={isSavingDownloadDirectory}
        onCookieSourceChange={handleCookieSourceChange}
        onCookieFilePathChange={handleCookieFilePathChange}
        onInstagramSessionIdChange={handleInstagramSessionIdChange}
        onInstagramCookieFilePathChange={handleInstagramCookieFilePathChange}
        onInstagramCollectModeChange={handleInstagramCollectModeChange}
        onInstagramCollectCountChange={handleInstagramCollectCountChange}
        onDownloadDirectoryDraftChange={setDownloadDirectoryDraft}
        onSaveDownloadDirectory={handleSaveDownloadDirectory}
        onResetDownloadDirectory={handleResetDownloadDirectory}
      />
    </>
  );
}
