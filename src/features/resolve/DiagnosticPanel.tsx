import { Button, Text } from '@react-spectrum/s2';
import type {
  DiagnoseMediaResponse,
  DiagnosticComparisonResult,
} from '../../lib/types';

type DiagnosticPanelProps = {
  result: DiagnoseMediaResponse;
  previousResult?: DiagnoseMediaResponse | null;
  comparison?: DiagnosticComparisonResult | null;
  isDiagnosing: boolean;
  onCopyCommand: () => void;
  onApplyResolved: () => void;
};

function yesNo(value: boolean): string {
  return value ? '是' : '否';
}

function formatHeight(height?: number | null): string {
  return height ? `${height}p` : '未知';
}

function titleOf(result: DiagnoseMediaResponse): string {
  return result.resolved?.title ?? '未解析成功';
}

export function DiagnosticPanel(props: DiagnosticPanelProps) {
  const { diagnostics } = props.result;

  return (
    <div className="paper-card diagnostic-card">
      <div className="section-head">
        <Text UNSAFE_className="section-kicker">诊断解析</Text>
        <div className="action-row">
          <Button
            variant="secondary"
            onPress={props.onCopyCommand}
            isDisabled={props.isDiagnosing}
          >
            复制 CLI 命令
          </Button>
          <Button
            variant="accent"
            onPress={props.onApplyResolved}
            isDisabled={!props.result.resolved || props.isDiagnosing}
          >
            用作解析结果
          </Button>
        </div>
      </div>

      <div className="diagnostic-summary">
        <div className="diagnostic-main">
          <Text UNSAFE_className="diagnostic-title">{titleOf(props.result)}</Text>
          <div className="result-meta">
            <Text UNSAFE_className="meta-tag">Cookie {diagnostics.cookieMode}</Text>
            <Text UNSAFE_className="meta-tag">yt-dlp {diagnostics.ytDlpSource}</Text>
            <Text UNSAFE_className="meta-tag">ffmpeg {diagnostics.ffmpegSource}</Text>
            <Text UNSAFE_className="meta-tag">
              代理 {diagnostics.proxyEnabled ? '已启用' : '未启用'}
            </Text>
          </div>
        </div>
        {diagnostics.errorCategory ? (
          <Text UNSAFE_className="diagnostic-error">
            {diagnostics.errorCategory} · {diagnostics.normalizedMessage ?? '诊断失败'}
          </Text>
        ) : (
          <Text UNSAFE_className="diagnostic-ok">
            已拿到 {diagnostics.formatsCount} 个格式，推荐 {formatHeight(diagnostics.bestHeight)}，
            最高 {formatHeight(diagnostics.maxHeight)}。
          </Text>
        )}
      </div>

      <div className="diagnostic-grid">
        <div className="diagnostic-metric">
          <Text UNSAFE_className="diagnostic-label">格式数量</Text>
          <Text UNSAFE_className="diagnostic-value">{diagnostics.formatsCount}</Text>
        </div>
        <div className="diagnostic-metric">
          <Text UNSAFE_className="diagnostic-label">推荐格式</Text>
          <Text UNSAFE_className="diagnostic-value">
            {diagnostics.bestFormatId ?? '未知'}
          </Text>
        </div>
        <div className="diagnostic-metric">
          <Text UNSAFE_className="diagnostic-label">推荐分辨率</Text>
          <Text UNSAFE_className="diagnostic-value">{formatHeight(diagnostics.bestHeight)}</Text>
        </div>
        <div className="diagnostic-metric">
          <Text UNSAFE_className="diagnostic-label">最高分辨率</Text>
          <Text UNSAFE_className="diagnostic-value">{formatHeight(diagnostics.maxHeight)}</Text>
        </div>
        <div className="diagnostic-metric">
          <Text UNSAFE_className="diagnostic-label">推荐含音轨</Text>
          <Text UNSAFE_className="diagnostic-value">{yesNo(diagnostics.bestHasAudio)}</Text>
        </div>
        <div className="diagnostic-metric">
          <Text UNSAFE_className="diagnostic-label">存在合流格式</Text>
          <Text UNSAFE_className="diagnostic-value">{yesNo(diagnostics.hasMuxedFormat)}</Text>
        </div>
        <div className="diagnostic-metric">
          <Text UNSAFE_className="diagnostic-label">存在纯视频</Text>
          <Text UNSAFE_className="diagnostic-value">{yesNo(diagnostics.hasVideoOnlyFormat)}</Text>
        </div>
        <div className="diagnostic-metric">
          <Text UNSAFE_className="diagnostic-label">存在纯音频</Text>
          <Text UNSAFE_className="diagnostic-value">{yesNo(diagnostics.hasAudioOnlyFormat)}</Text>
        </div>
      </div>

      {props.comparison ? (
        <div className="diagnostic-compare">
          <Text UNSAFE_className="diagnostic-label">
            对比上一次（{props.previousResult?.diagnostics.cookieMode ?? '未知'}）
          </Text>
          <Text UNSAFE_className="diagnostic-compare-message">{props.comparison.message}</Text>
        </div>
      ) : null}

      <div className="diagnostic-command-block">
        <Text UNSAFE_className="diagnostic-label">CLI 复现命令</Text>
        <pre className="diagnostic-command">{diagnostics.commandPreview.displayCommand}</pre>
        {diagnostics.rawErrorMessage &&
        diagnostics.rawErrorMessage !== diagnostics.normalizedMessage ? (
          <Text UNSAFE_className="diagnostic-raw-error">
            原始错误：{diagnostics.rawErrorMessage}
          </Text>
        ) : null}
      </div>
    </div>
  );
}
