import { useState } from 'react';
import { Button, Heading, Picker, PickerItem, StatusLight, Text, TextField } from '@react-spectrum/s2';
import { CookieSourcePanel } from '../cookies/CookieSourcePanel';
import { SessionIdHelpDrawer } from './SessionIdHelpDrawer';
import type {
  CookieSource,
  DependencyStatus,
  DownloadDirectorySettings,
  InstagramCollectMode,
} from '../../lib/types';
import { MAX_DOWNLOAD_CONCURRENCY, MIN_DOWNLOAD_CONCURRENCY } from '../../lib/settings';

const INSTAGRAM_MODE_OPTIONS: { id: InstagramCollectMode; label: string }[] = [
  { id: 'single', label: '当前链接（单条）' },
  { id: 'detail_next', label: '详情页连续下一条' },
  { id: 'profile_recent', label: '用户主页最近内容' },
  { id: 'story_experimental', label: 'Story（实验性）' },
];

const DOWNLOAD_CONCURRENCY_OPTIONS = Array.from(
  { length: MAX_DOWNLOAD_CONCURRENCY - MIN_DOWNLOAD_CONCURRENCY + 1 },
  (_item, index) => {
    const value = MIN_DOWNLOAD_CONCURRENCY + index;
    return { id: String(value), label: `${value} 个任务` };
  },
);

export type SettingsPanelProps = {
  cookieSources: CookieSource[];
  selectedCookieSource: string;
  cookieFilePath: string;
  instagramSessionId: string;
  instagramCookieFilePath: string;
  instagramCollectMode: InstagramCollectMode;
  instagramCollectCount: string;
  autoDownload: boolean;
  downloadConcurrency: number;
  dependencyStatus: DependencyStatus | null;
  downloadDirectory: DownloadDirectorySettings | null;
  downloadDirectoryDraft: string;
  isSavingDownloadDirectory: boolean;
  onCookieSourceChange: (key: string) => void;
  onCookieFilePathChange: (value: string) => void;
  onInstagramSessionIdChange: (value: string) => void;
  onInstagramCookieFilePathChange: (value: string) => void;
  onInstagramCollectModeChange: (value: InstagramCollectMode) => void;
  onInstagramCollectCountChange: (value: string) => void;
  onAutoDownloadChange: (value: boolean) => void;
  onDownloadConcurrencyChange: (value: number) => void;
  onDownloadDirectoryDraftChange: (value: string) => void;
  onSaveDownloadDirectory: () => void;
  onResetDownloadDirectory: () => void;
};

export function SettingsPanel(props: SettingsPanelProps) {
  const [isSessionIdHelpOpen, setIsSessionIdHelpOpen] = useState(false);

  return (
          <div className="panel-stack settings-panel-stack">
            <CookieSourcePanel
              items={props.cookieSources}
              selectedKey={props.selectedCookieSource}
              cookieFilePath={props.cookieFilePath}
              onSelectionChange={props.onCookieSourceChange}
              onCookieFilePathChange={props.onCookieFilePathChange}
            />
            <div className="instagram-settings panel-stack">
              <div className="instagram-settings-head">
                <Heading level={4} UNSAFE_className="settings-subtitle">Instagram 访问</Heading>
                <Button
                  variant="secondary"
                  onPress={() => setIsSessionIdHelpOpen(true)}
                >
                  如何获取？
                </Button>
              </div>
              <SessionIdHelpDrawer
                open={isSessionIdHelpOpen}
                onClose={() => setIsSessionIdHelpOpen(false)}
              />
              <Text UNSAFE_className="settings-hint">
                主推荐粘贴 sessionid；cookies.txt 作为备用登录方案。
              </Text>
              <TextField
                label="Instagram sessionid"
                type="password"
                value={props.instagramSessionId}
                onChange={props.onInstagramSessionIdChange}
                placeholder="粘贴 sessionid，主推荐方案"
                UNSAFE_style={{ width: '100%' }}
              />
              <TextField
                label="Instagram cookies.txt 路径（备用）"
                value={props.instagramCookieFilePath}
                onChange={props.onInstagramCookieFilePathChange}
                placeholder="例如 C:\\Users\\Administrator\\Downloads\\instagram-cookies.txt"
                UNSAFE_style={{ width: '100%' }}
              />
              <Picker
                label="采集模式"
                selectedKey={props.instagramCollectMode}
                onSelectionChange={(key) =>
                  props.onInstagramCollectModeChange(key as InstagramCollectMode)
                }
                items={INSTAGRAM_MODE_OPTIONS}
              >
                {(item) => <PickerItem>{item.label}</PickerItem>}
              </Picker>
              <TextField
                label="抓取数量"
                value={props.instagramCollectCount}
                onChange={props.onInstagramCollectCountChange}
                inputMode="numeric"
                placeholder="例如 1、3、5"
              />
            </div>
            <label className="auto-download-toggle">
              <span className="auto-download-toggle-text">
                <Text UNSAFE_className="auto-download-label">自动下载</Text>
                <Text UNSAFE_className="auto-download-hint">
                  解析完成后自动以所选画质加入下载队列
                </Text>
              </span>
              <input
                type="checkbox"
                className="auto-download-switch"
                checked={props.autoDownload}
                onChange={(event) => props.onAutoDownloadChange(event.target.checked)}
              />
            </label>
            <Picker
              label="同时下载数"
              selectedKey={String(props.downloadConcurrency)}
              onSelectionChange={(key) => props.onDownloadConcurrencyChange(Number(key))}
              items={DOWNLOAD_CONCURRENCY_OPTIONS}
            >
              {(item) => <PickerItem>{item.label}</PickerItem>}
            </Picker>
            <div className="download-dir-settings">
              <TextField
                label="下载目录"
                value={props.downloadDirectoryDraft}
                onChange={props.onDownloadDirectoryDraftChange}
                placeholder={props.downloadDirectory?.defaultDir ?? '输入下载目录'}
                description={
                  props.downloadDirectory?.isCustom
                    ? `当前使用自定义目录，默认目录是 ${props.downloadDirectory.defaultDir}。下载会按「网站/类型(视频·图片·音频)」分文件夹存放。`
                    : '当前使用默认目录；下载会按「网站/类型(视频·图片·音频)」分文件夹存放，失败下载保留在 incomplete 文件夹里。'
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
  );
}
