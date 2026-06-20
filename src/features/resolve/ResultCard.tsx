import { Button, Heading, Text } from '@react-spectrum/s2';
import type { MediaFormat, ResolveMediaResponse } from '../../lib/types';

type ResultCardProps = {
  resolved: ResolveMediaResponse | null;
  thumbnail: string | null;
  isPreviewLoading: boolean;
  downloadingIds: ReadonlySet<string>;
  onDownload: (format: MediaFormat) => void;
};

function formatBytes(bytes?: number | null): string {
  if (!bytes || bytes <= 0) {
    return '未知大小';
  }
  const units = ['B', 'KB', 'MB', 'GB'];
  let value = bytes;
  let unitIndex = 0;
  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }
  return `${value.toFixed(value >= 100 || unitIndex === 0 ? 0 : 1)} ${units[unitIndex]}`;
}

export function ResultCard({
  resolved,
  thumbnail,
  isPreviewLoading,
  downloadingIds,
  onDownload,
}: ResultCardProps) {
  if (!resolved) {
    return null;
  }

  return (
    <div className="paper-card result-card">
      <Heading level={3} UNSAFE_className="card-title">{resolved.title}</Heading>
      <div className="card-body">
          <div className="preview-frame">
            {thumbnail ? (
              <img className="preview-image" src={thumbnail} alt="视频预览" />
            ) : (
              <div className="preview-placeholder">
                <Text>{isPreviewLoading ? '正在生成预览…' : '无预览'}</Text>
              </div>
            )}
          </div>

          <div className="result-meta">
            <Text UNSAFE_className="meta-tag">{resolved.source}</Text>
            <Text UNSAFE_className="meta-tag">时长 {resolved.durationText}</Text>
            <Text UNSAFE_className="meta-tag">{resolved.formats.length} 个版本</Text>
          </div>

          <Text UNSAFE_className="format-list-title">选择要下载的版本</Text>
          <ul className="format-list">
            {resolved.formats.map((format) => {
              const isDownloading = downloadingIds.has(format.id);
              return (
                <li key={format.id} className="format-row">
                  <div className="format-info">
                    <Text UNSAFE_className="format-label">{format.label}</Text>
                    <Text UNSAFE_className="format-sub">
                      {format.ext.toUpperCase()} · {formatBytes(format.sizeBytes)}
                      {format.hasAudio ? ' · 含音轨' : ' · 无音轨'}
                    </Text>
                  </div>
                  <Button
                    variant="accent"
                    onPress={() => onDownload(format)}
                    isPending={isDownloading}
                  >
                    下载
                  </Button>
                </li>
              );
            })}
          </ul>
      </div>
    </div>
  );
}
