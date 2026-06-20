import { Cell, Column, Row, TableBody, TableHeader, TableView } from '@react-spectrum/s2';

export type DownloadRow = {
  id: string;
  title: string;
  status: string;
  progress: string;
  speed?: string;
};

export function DownloadsTable({ rows }: { rows: DownloadRow[] }) {
  return (
    <div className="downloads-table">
      <TableView aria-label="下载队列">
        <TableHeader>
          <Column key="id">ID</Column>
          <Column key="title">标题</Column>
          <Column key="status">状态</Column>
          <Column key="progress">进度</Column>
        </TableHeader>
        <TableBody>
          {rows.map((row) => (
            <Row key={row.id}>
              <Cell>{row.id}</Cell>
              <Cell>{row.title}</Cell>
              <Cell>{row.status}</Cell>
              <Cell>{row.progress}</Cell>
            </Row>
          ))}
        </TableBody>
      </TableView>
    </div>
  );
}
