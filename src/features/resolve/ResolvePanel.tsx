import { Button, TextField } from '@react-spectrum/s2';

type ResolvePanelProps = {
  url: string;
  isResolving: boolean;
  onUrlChange: (value: string) => void;
  onDownloadBest: () => void;
  onShowFormats: () => void;
};

export function ResolvePanel(props: ResolvePanelProps) {
  return (
    <section className="panel-stack">
      <TextField
        label="视频地址"
        value={props.url}
        onChange={props.onUrlChange}
        placeholder="粘贴 x.com 或 pornhub.com 视频页面地址"
      />
      <div className="action-row">
        <Button variant="accent" onPress={props.onDownloadBest} isPending={props.isResolving}>
          下载最佳
        </Button>
        <Button variant="secondary" onPress={props.onShowFormats} isPending={props.isResolving}>
          查看更多格式
        </Button>
      </div>
    </section>
  );
}
