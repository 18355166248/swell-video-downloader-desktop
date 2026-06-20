import { Button, Heading, InlineAlert, Text } from '@react-spectrum/s2';
import { useEffect, useState } from 'react';
import { DownloadsTable, type DownloadRow } from './features/downloads/DownloadsTable';
import { ResolvePanel } from './features/resolve/ResolvePanel';
import { ResultCard } from './features/resolve/ResultCard';
import { SettingsPanel } from './features/settings/SettingsPanel';
import {
  listenDownloadError,
  listenDownloadProgress,
  listenDownloadStatus,
} from './lib/download-events';
import {
  checkDependencies,
  generatePreview,
  getDownloadDirectorySettings,
  listCookieSources,
  openDownloadLocation,
  resetDownloadDir,
  resolveMedia,
  setDownloadDir,
  startDownload,
} from './lib/tauri';
import type {
  CookieSource,
  DependencyStatus,
  DownloadDirectorySettings,
  MediaFormat,
  ResolveMediaResponse,
} from './lib/types';

type DownloadBucketState = {
  current: DownloadRow[];
  history: DownloadRow[];
};

type DownloadTab = 'current' | 'history';

const DOWNLOAD_HISTORY_STORAGE_KEY = 'swell.downloadHistory.v1';

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

function archiveCurrentRows(state: DownloadBucketState): DownloadBucketState {
  if (state.current.length === 0) {
    return state;
  }

  return {
    current: [],
    history: mergeRowsById(state.current, state.history),
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

export default function App() {
  const [url, setUrl] = useState('');
  const [resolved, setResolved] = useState<ResolveMediaResponse | null>(null);
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
  const [dependencyStatus, setDependencyStatus] = useState<DependencyStatus | null>(null);
  const [thumbnail, setThumbnail] = useState<string | null>(null);
  const [isPreviewLoading, setIsPreviewLoading] = useState(false);
  const [downloadingIds, setDownloadingIds] = useState<Set<string>>(new Set());
  const [currentSessionLabel, setCurrentSessionLabel] = useState('本次解析');

  useEffect(() => {
    async function loadSettingsData() {
      const [sources, dependencies, dirSettings] = await Promise.all([
        listCookieSources(),
        checkDependencies(),
        getDownloadDirectorySettings(),
      ]);
      setCookieSources(sources);
      if (sources.length > 0) {
        setSelectedCookieSource(sources[0].id);
      }
      setDependencyStatus(dependencies);
      setDownloadDirectorySettings(dirSettings);
      setDownloadDirState(dirSettings.currentDir);
      setDownloadDirectoryDraft(dirSettings.currentDir);
    }

    loadSettingsData().catch((err) => {
      setError(err instanceof Error ? err.message : '初始化设置数据失败');
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
        if (payload.message) {
          setError(payload.message);
        }
      }),
    ])
      .then((handlers) => {
        handlers.forEach((handler) => unlisteners.push(handler));
      })
      .catch((err) => {
        setError(err instanceof Error ? err.message : '注册下载事件失败');
      });

    return () => {
      unlisteners.forEach((dispose) => dispose());
    };
  }, []);

  async function loadPreview(trimmedUrl: string, result: ResolveMediaResponse) {
    // yt-dlp paths come with a poster URL already; only the ssstwitter fallback
    // needs an ffmpeg-generated frame. Use the smallest variant for speed.
    if (result.thumbnail) {
      setThumbnail(result.thumbnail);
      return;
    }

    const previewFormat = result.formats[result.formats.length - 1] ?? result.recommendation;
    if (!previewFormat) {
      return;
    }

    setIsPreviewLoading(true);
    try {
      const dataUrl = await generatePreview(trimmedUrl, previewFormat.id);
      setThumbnail(dataUrl);
    } catch {
      setThumbnail(null);
    } finally {
      setIsPreviewLoading(false);
    }
  }

  async function handleResolve() {
    const trimmedUrl = url.trim();
    if (!trimmedUrl) {
      setResolved(null);
      setError('请先输入视频地址。');
      return;
    }

    setIsResolving(true);
    setError('');
    setThumbnail(null);
    setResolved(null);
    setDownloadTab('current');
    setDownloadingIds(new Set());
    setDownloadState((current) => archiveCurrentRows(current));

    try {
      const result = await resolveMedia(trimmedUrl, selectedCookieSource, cookieFilePath);
      setResolved(result);
      setCurrentSessionLabel(deriveSessionLabel(trimmedUrl, result));
      void loadPreview(trimmedUrl, result);
    } catch (err) {
      setResolved(null);
      setError(err instanceof Error ? err.message : '解析失败');
    } finally {
      setIsResolving(false);
    }
  }

  async function handleDownloadFormat(format: MediaFormat) {
    const trimmedUrl = url.trim();
    if (!trimmedUrl || !resolved) {
      return;
    }

    setError('');
    setDownloadingIds((current) => new Set(current).add(format.id));
    try {
      const taskTitle = buildDownloadTitle(resolved.title, format);
      const taskId = await startDownload(
        trimmedUrl,
        format.id ?? null,
        taskTitle,
        selectedCookieSource,
        cookieFilePath,
      );
      setDownloadState((current) => ({
        ...current,
        current: [
          {
            id: taskId,
            title: taskTitle,
            sessionLabel: currentSessionLabel,
            status: 'queued',
            progress: '0%',
          },
          ...current.current,
        ],
      }));
    } catch (err) {
      setError(err instanceof Error ? err.message : '创建下载任务失败');
    } finally {
      setDownloadingIds((current) => {
        const next = new Set(current);
        next.delete(format.id);
        return next;
      });
    }
  }

  async function handleOpenLocation(row: DownloadRow) {
    try {
      await openDownloadLocation(downloadDir, row.outputPath);
    } catch (err) {
      setError(err instanceof Error ? err.message : '打开文件夹失败');
    }
  }

  async function handleOpenDownloadDir() {
    if (!downloadDir) {
      return;
    }
    try {
      await openDownloadLocation(downloadDir);
    } catch (err) {
      setError(err instanceof Error ? err.message : '打开下载目录失败');
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
    } catch (err) {
      setError(err instanceof Error ? err.message : '保存下载目录失败');
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
    } catch (err) {
      setError(err instanceof Error ? err.message : '恢复默认下载目录失败');
    } finally {
      setIsSavingDownloadDirectory(false);
    }
  }

  return (
    <main className="app-shell">
      <section className="hero-block">
        <Text UNSAFE_className="eyebrow">桌面主线项目</Text>
        <Heading level={1} UNSAFE_className="hero-title">
          Swell <em>Video</em> Downloader
        </Heading>
        <Text UNSAFE_className="hero-sub">
          输入视频页 URL，先解析查看预览与各清晰度大小，再自行选择要下载的版本。
        </Text>
      </section>

      <div className="flow">
        <section className="flow-step" data-step="01" aria-label="解析">
          <ResolvePanel
            url={url}
            isResolving={isResolving}
            onUrlChange={setUrl}
            onResolve={handleResolve}
          />
          {error ? <InlineAlert variant="negative">{error}</InlineAlert> : null}
        </section>

        {resolved ? (
          <section className="flow-step" data-step="02" aria-label="选择版本">
            <ResultCard
              resolved={resolved}
              thumbnail={thumbnail}
              isPreviewLoading={isPreviewLoading}
              downloadingIds={downloadingIds}
              onDownload={handleDownloadFormat}
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
              ariaLabel="当前下载队列"
              emptyText="解析新视频后，这里只展示当前流程的下载任务。"
              rows={downloadState.current}
              onOpenLocation={handleOpenLocation}
            />
          ) : (
            <DownloadsTable
              ariaLabel="下载历史"
              emptyText="还没有历史记录。完成或失败过的任务会出现在这里。"
              rows={downloadState.history}
              onOpenLocation={handleOpenLocation}
            />
          )}
        </section>

        <section className="flow-step" data-step="04" aria-label="设置">
          <SettingsPanel
            cookieSources={cookieSources}
            selectedCookieSource={selectedCookieSource}
            cookieFilePath={cookieFilePath}
            dependencyStatus={dependencyStatus}
            downloadDirectory={downloadDirectorySettings}
            downloadDirectoryDraft={downloadDirectoryDraft}
            isSavingDownloadDirectory={isSavingDownloadDirectory}
            onCookieSourceChange={setSelectedCookieSource}
            onCookieFilePathChange={setCookieFilePath}
            onDownloadDirectoryDraftChange={setDownloadDirectoryDraft}
            onSaveDownloadDirectory={handleSaveDownloadDirectory}
            onResetDownloadDirectory={handleResetDownloadDirectory}
          />
        </section>
      </div>
    </main>
  );
}
