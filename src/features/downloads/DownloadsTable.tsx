import { ActionButton, Cell, Column, Row, TableBody, TableHeader, TableView } from '@react-spectrum/s2';

export type DownloadRow = {
  id: string;
  title: string;
  status: string;
  progress: string;
  speed?: string;
  outputPath?: string | null;
  sessionLabel?: string;
};

type DownloadsTableProps = {
  mode: 'current' | 'history';
  rows: DownloadRow[];
  onOpenLocation: (row: DownloadRow) => void;
  onCancel?: (row: DownloadRow) => void;
  onDelete?: (row: DownloadRow) => void;
  emptyText: string;
  ariaLabel: string;
};

function formatStatus(status: string): string {
  switch (status) {
    case 'queued':
      return '排队中';
    case 'downloading':
      return '下载中';
    case 'postprocessing':
      return '处理中';
    case 'completed':
      return '已完成';
    case 'failed':
      return '失败';
    case 'canceling':
      return '取消中';
    case 'canceled':
      return '已取消';
    default:
      return status;
  }
}

function canCancel(status: string) {
  return ['queued', 'downloading', 'postprocessing', 'canceling'].includes(status);
}

export function DownloadsTable({
  mode,
  rows,
  onOpenLocation,
  onCancel,
  onDelete,
  emptyText,
  ariaLabel,
}: DownloadsTableProps) {
  return (
    <div className="downloads-table">
      <TableView aria-label={ariaLabel}>
        <TableHeader>
          <Column key="title" isRowHeader>标题</Column>
          <Column key="session">来源</Column>
          <Column key="status">状态</Column>
          <Column key="progress">进度</Column>
          <Column key="speed">速度</Column>
          <Column key="actions">操作</Column>
        </TableHeader>
        <TableBody>
          {rows.length === 0 ? (
            <Row>
              <Cell>{emptyText}</Cell>
              <Cell>--</Cell>
              <Cell>--</Cell>
              <Cell>--</Cell>
              <Cell>--</Cell>
              <Cell>--</Cell>
            </Row>
          ) : (
            rows.map((row) => (
              <Row key={row.id}>
                <Cell>{row.title}</Cell>
                <Cell>{row.sessionLabel ?? '--'}</Cell>
                <Cell>{formatStatus(row.status)}</Cell>
                <Cell>{row.progress}</Cell>
                <Cell>{row.speed ?? '--'}</Cell>
                <Cell>
                  <div className="table-actions">
                    <ActionButton onPress={() => onOpenLocation(row)}>
                      {row.outputPath ? '打开所在文件夹' : '打开下载目录'}
                    </ActionButton>
                    {mode === 'current' && onCancel && canCancel(row.status) ? (
                      <ActionButton onPress={() => onCancel(row)}>
                        取消下载
                      </ActionButton>
                    ) : null}
                    {mode === 'history' && onDelete ? (
                      <ActionButton onPress={() => onDelete(row)}>
                        删除记录
                      </ActionButton>
                    ) : null}
                  </div>
                </Cell>
              </Row>
            ))
          )}
        </TableBody>
      </TableView>
    </div>
  );
}
