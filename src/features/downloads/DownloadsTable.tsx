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
  rows: DownloadRow[];
  onOpenLocation: (row: DownloadRow) => void;
  emptyText: string;
  ariaLabel: string;
};

export function DownloadsTable({ rows, onOpenLocation, emptyText, ariaLabel }: DownloadsTableProps) {
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
                <Cell>{row.status}</Cell>
                <Cell>{row.progress}</Cell>
                <Cell>{row.speed ?? '--'}</Cell>
                <Cell>
                  <ActionButton onPress={() => onOpenLocation(row)}>
                    {row.outputPath ? '打开所在文件夹' : '打开下载目录'}
                  </ActionButton>
                </Cell>
              </Row>
            ))
          )}
        </TableBody>
      </TableView>
    </div>
  );
}
