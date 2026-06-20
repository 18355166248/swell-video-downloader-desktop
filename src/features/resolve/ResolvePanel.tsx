import { Button, Text, TextField } from '@react-spectrum/s2';

type ResolvePanelProps = {
  url: string;
  batchUrls: string;
  isResolving: boolean;
  isBatchDownloading: boolean;
  onUrlChange: (value: string) => void;
  onBatchUrlsChange: (value: string) => void;
  onResolve: () => void;
  onBatchDownload: () => void;
};

export function ResolvePanel(props: ResolvePanelProps) {
  return (
    <div className="paper-card resolve-card">
      <TextField
        label="视频地址"
        value={props.url}
        onChange={props.onUrlChange}
        placeholder="粘贴 x.com 或 pornhub.com 视频页面地址"
        UNSAFE_style={{ width: '100%' }}
      />
      <div className="action-row">
        <Button variant="accent" onPress={props.onResolve} isPending={props.isResolving}>
          解析视频
        </Button>
      </div>
      <div className="batch-panel">
        <Text UNSAFE_className="batch-panel-title">批量粘贴链接并排队下载</Text>
        <Text UNSAFE_className="batch-panel-sub">
          一行一个链接。第一版会先解析每条链接，再自动下载推荐版本。
        </Text>
        <label className="batch-textarea-wrap">
          <span className="batch-textarea-label">批量链接</span>
          <textarea
            className="batch-textarea"
            value={props.batchUrls}
            onChange={(event) => props.onBatchUrlsChange(event.target.value)}
            placeholder={'https://x.com/...\nhttps://www.pornhub.com/...'}
            rows={5}
          />
        </label>
        <div className="action-row">
          <Button
            variant="secondary"
            onPress={props.onBatchDownload}
            isPending={props.isBatchDownloading}
          >
            批量下载推荐版本
          </Button>
        </div>
      </div>
    </div>
  );
}
