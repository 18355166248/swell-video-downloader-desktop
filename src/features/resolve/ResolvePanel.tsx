import { Button, ProgressCircle, Text } from '@react-spectrum/s2';

type ResolvePanelProps = {
  urls: string;
  urlCount: number;
  isResolving: boolean;
  isDiagnosing: boolean;
  onUrlsChange: (value: string) => void;
  onSubmit: () => void;
  onDiagnose: () => void;
};

export function ResolvePanel(props: ResolvePanelProps) {
  const isBatch = props.urlCount > 1;
  const buttonLabel = isBatch ? `解析 ${props.urlCount} 个视频` : '解析视频';

  return (
    <div className="paper-card intro-card">
      <div className="intro-head">
        <Text UNSAFE_className="section-kicker">开始下载</Text>
        <Text UNSAFE_className="intro-tagline">
          粘贴视频页地址即可。一行一个链接，解析后会列出每个视频，逐个选择清晰度再下载。
        </Text>
      </div>

      <label className="intro-input-wrap">
        <textarea
          className="batch-textarea intro-textarea"
          value={props.urls}
          onChange={(event) => props.onUrlsChange(event.target.value)}
          placeholder={'粘贴 x.com / pornhub.com 视频页地址\n多条则每行一个'}
          rows={4}
        />
        {props.urlCount > 0 ? (
          <span className="intro-input-count">{props.urlCount} 条链接</span>
        ) : null}
      </label>

      <div className="action-row intro-actions">
        {props.isResolving ? (
          <ProgressCircle aria-label="正在解析" size="S" isIndeterminate />
        ) : null}
        {props.isDiagnosing ? (
          <ProgressCircle aria-label="正在诊断" size="S" isIndeterminate />
        ) : null}
        {/* While resolving, the button stays pressable and flips to a cancel action. */}
        <Button
          variant="secondary"
          onPress={props.onDiagnose}
          isDisabled={props.isResolving}
        >
          诊断解析
        </Button>
        <Button
          variant={props.isResolving ? 'negative' : 'accent'}
          onPress={props.onSubmit}
        >
          {props.isResolving ? '取消解析' : buttonLabel}
        </Button>
      </div>
    </div>
  );
}
