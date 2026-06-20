import { Button, Heading, StatusLight, Text, TextField } from '@react-spectrum/s2';
import { CookieSourcePanel } from '../cookies/CookieSourcePanel';
import type { CookieSource, DependencyStatus, DownloadDirectorySettings } from '../../lib/types';

type SettingsPanelProps = {
  cookieSources: CookieSource[];
  selectedCookieSource: string;
  cookieFilePath: string;
  dependencyStatus: DependencyStatus | null;
  downloadDirectory: DownloadDirectorySettings | null;
  downloadDirectoryDraft: string;
  isSavingDownloadDirectory: boolean;
  onCookieSourceChange: (key: string) => void;
  onCookieFilePathChange: (value: string) => void;
  onDownloadDirectoryDraftChange: (value: string) => void;
  onSaveDownloadDirectory: () => void;
  onResetDownloadDirectory: () => void;
};

export function SettingsPanel(props: SettingsPanelProps) {
  return (
    <div className="paper-card settings-card">
      <Heading level={3} UNSAFE_className="card-title">设置</Heading>
      <div className="card-body">
          <div className="panel-stack">
            <CookieSourcePanel
              items={props.cookieSources}
              selectedKey={props.selectedCookieSource}
              cookieFilePath={props.cookieFilePath}
              onSelectionChange={props.onCookieSourceChange}
              onCookieFilePathChange={props.onCookieFilePathChange}
            />
            <div className="download-dir-settings">
              <TextField
                label="下载目录"
                value={props.downloadDirectoryDraft}
                onChange={props.onDownloadDirectoryDraftChange}
                placeholder={props.downloadDirectory?.defaultDir ?? '输入下载目录'}
                description={
                  props.downloadDirectory?.isCustom
                    ? `当前使用自定义目录，默认目录是 ${props.downloadDirectory.defaultDir}`
                    : '当前使用默认目录；失败下载会保留在该目录下的 incomplete 文件夹里。'
                }
                UNSAFE_style={{ width: '100%' }}
              />
              <div className="action-row">
                <Button
                  variant="accent"
                  onPress={props.onSaveDownloadDirectory}
                  isPending={props.isSavingDownloadDirectory}
                >
                  保存下载目录
                </Button>
                <Button
                  variant="secondary"
                  onPress={props.onResetDownloadDirectory}
                  isPending={props.isSavingDownloadDirectory}
                >
                  恢复默认目录
                </Button>
              </div>
            </div>
            <div className="dependency-grid">
              <StatusLight variant={props.dependencyStatus?.ytDlpOk ? 'positive' : 'negative'}>
                yt-dlp ({props.dependencyStatus?.ytDlpSource ?? 'unknown'})
              </StatusLight>
              <StatusLight variant={props.dependencyStatus?.ffmpegOk ? 'positive' : 'negative'}>
                ffmpeg ({props.dependencyStatus?.ffmpegSource ?? 'unknown'})
              </StatusLight>
            </div>
            <Text>默认优先使用项目内置二进制，其次回退到系统 PATH。</Text>
            <Text>手动导入时请提供 Netscape 格式的 cookies.txt，并确保该 Cookie 能在浏览器里正常查看目标 X 内容。</Text>
          </div>
      </div>
    </div>
  );
}
