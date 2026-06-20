import { Card, Content, Heading, Text } from '@react-spectrum/s2';

export type ResolvedSummary = {
  title: string;
  source: string;
  durationText: string;
  recommendation: string;
};

export function ResultCard({ summary }: { summary: ResolvedSummary | null }) {
  if (!summary) {
    return null;
  }

  return (
    <div className="result-card">
      <Card>
        <Heading level={3}>{summary.title}</Heading>
        <Content>
          <div className="result-meta">
            <Text>{summary.source}</Text>
            <Text>{summary.durationText}</Text>
            <Text>推荐：{summary.recommendation}</Text>
          </div>
        </Content>
      </Card>
    </div>
  );
}
