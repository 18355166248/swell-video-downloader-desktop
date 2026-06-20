import { Card, Content, Heading, StatusLight, Text } from '@react-spectrum/s2';
import { CookieSourcePanel } from '../cookies/CookieSourcePanel';
import type { CookieSource, DependencyStatus } from '../../lib/types';

type SettingsPanelProps = {
  cookieSources: CookieSource[];
  selectedCookieSource: string;
  cookieFilePath: string;
  dependencyStatus: DependencyStatus | null;
  onCookieSourceChange: (key: string) => void;
  onCookieFilePathChange: (value: string) => void;
};

export function SettingsPanel(props: SettingsPanelProps) {
  return (
    <div className="settings-card">
      <Card>
        <Heading level={3}>设置</Heading>
        <Content>
          <div className="panel-stack">
            <CookieSourcePanel
              items={props.cookieSources}
              selectedKey={props.selectedCookieSource}
              cookieFilePath={props.cookieFilePath}
              onSelectionChange={props.onCookieSourceChange}
              onCookieFilePathChange={props.onCookieFilePathChange}
            />
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
        </Content>
      </Card>
    </div>
  );
}
