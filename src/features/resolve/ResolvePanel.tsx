import { Button, TextField } from '@react-spectrum/s2';

type ResolvePanelProps = {
  url: string;
  isResolving: boolean;
  onUrlChange: (value: string) => void;
  onResolve: () => void;
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
    </div>
  );
}
