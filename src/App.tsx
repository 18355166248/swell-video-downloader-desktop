import { Heading, InlineAlert, Text } from '@react-spectrum/s2';
import { useEffect, useState } from 'react';
import { DownloadsTable } from './features/downloads/DownloadsTable';
import { ResolvePanel } from './features/resolve/ResolvePanel';
import { ResultCard } from './features/resolve/ResultCard';
import { SettingsPanel } from './features/settings/SettingsPanel';
import {
  listenDownloadError,
  listenDownloadProgress,
  listenDownloadStatus,
} from './lib/download-events';
import { checkDependencies, listCookieSources, resolveMedia, startDownload } from './lib/tauri';
import type { CookieSource, DependencyStatus, ResolveMediaResponse } from './lib/types';

export default function App() {
  const [url, setUrl] = useState('');
  const [resolved, setResolved] = useState<ResolveMediaResponse | null>(null);
  const [error, setError] = useState('');
  const [isResolving, setIsResolving] = useState(false);
  const [downloadRows, setDownloadRows] = useState<Array<{
    id: string;
    title: string;
    status: string;
    progress: string;
    speed?: string;
  }>>([]);
  const [cookieSources, setCookieSources] = useState<CookieSource[]>([]);
  const [selectedCookieSource, setSelectedCookieSource] = useState('chrome');
  const [cookieFilePath, setCookieFilePath] = useState('');
  const [dependencyStatus, setDependencyStatus] = useState<DependencyStatus | null>(null);

  useEffect(() => {
    async function loadSettingsData() {
      const [sources, dependencies] = await Promise.all([
        listCookieSources(),
        checkDependencies(),
      ]);
      setCookieSources(sources);
      if (sources.length > 0) {
        setSelectedCookieSource(sources[0].id);
      }
      setDependencyStatus(dependencies);
    }

    loadSettingsData().catch((err) => {
      setError(err instanceof Error ? err.message : '初始化设置数据失败');
    });
  }, []);

  useEffect(() => {
    const unlisteners: Array<() => void> = [];

    Promise.all([
      listenDownloadProgress((payload) => {
        setDownloadRows((current) =>
          current.map((row) =>
            row.id === payload.task_id
              ? {
                  ...row,
                  progress: payload.percent,
                  speed: payload.speed,
                }
              : row,
          ),
        );
      }),
      listenDownloadStatus((payload) => {
        setDownloadRows((current) => {
          const existing = current.find((row) => row.id === payload.task_id);
          if (!existing) {
            return [
              {
                id: payload.task_id,
                title: payload.title,
                status: payload.status,
                progress: payload.status === 'completed' ? '100%' : '0%',
              },
              ...current,
            ];
          }

          return current.map((row) =>
            row.id === payload.task_id
              ? {
                  ...row,
                  title: payload.title,
                  status: payload.status,
                  progress:
                    payload.status === 'completed'
                      ? '100%'
                      : payload.status === 'failed'
                        ? row.progress || '0%'
                        : row.progress,
                }
              : row,
          );
        });
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

  async function handleResolve() {
    const trimmedUrl = url.trim();
    if (!trimmedUrl) {
      setResolved(null);
      setError('请先输入视频地址。');
      return null;
    }

    setIsResolving(true);
    setError('');

    try {
      const result = await resolveMedia(trimmedUrl, selectedCookieSource, cookieFilePath);
      setResolved(result);
      return result;
    } catch (err) {
      setResolved(null);
      setError(err instanceof Error ? err.message : '解析失败');
      return null;
    } finally {
      setIsResolving(false);
    }
  }

  async function handleDownloadBest() {
    const result = await handleResolve();

    if (!url.trim() || !result) {
      return;
    }

    try {
      const taskId = await startDownload(
        url.trim(),
        result.recommendation.id ?? null,
        result.title,
        selectedCookieSource,
        cookieFilePath,
      );
      setDownloadRows((current) => [
        {
          id: taskId,
          title: result.title,
          status: 'queued',
          progress: '0%',
        },
        ...current,
      ]);
    } catch (err) {
      setError(err instanceof Error ? err.message : '创建下载任务失败');
    }
  }

  return (
    <main className="app-shell">
      <section className="hero-block">
        <Text UNSAFE_className="eyebrow">桌面主线项目</Text>
        <Heading level={1}>Swell Video Downloader</Heading>
        <Text>输入视频页 URL，默认一键下载最佳版本，需要时再展开高级格式。</Text>
      </section>

      <section className="workspace-grid">
        <div className="left-column">
          <ResolvePanel
            url={url}
            isResolving={isResolving}
            onUrlChange={setUrl}
            onDownloadBest={handleDownloadBest}
            onShowFormats={handleResolve}
          />
          {error ? <InlineAlert variant="negative">{error}</InlineAlert> : null}
          <ResultCard
            summary={
              resolved
                ? {
                    title: resolved.title,
                    source: resolved.source,
                    durationText: resolved.durationText,
                    recommendation: `${resolved.recommendation.label} ${resolved.recommendation.ext}`,
                  }
                : null
            }
          />
        </div>

        <div className="right-column">
          <DownloadsTable rows={downloadRows} />
          <SettingsPanel
            cookieSources={cookieSources}
            selectedCookieSource={selectedCookieSource}
            cookieFilePath={cookieFilePath}
            dependencyStatus={dependencyStatus}
            onCookieSourceChange={setSelectedCookieSource}
            onCookieFilePathChange={setCookieFilePath}
          />
        </div>
      </section>
    </main>
  );
}
