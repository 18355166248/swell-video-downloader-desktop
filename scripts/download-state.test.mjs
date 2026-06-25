import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import ts from 'typescript';

const sourcePath = path.resolve('src/features/downloads/download-state.ts');
const source = fs.readFileSync(sourcePath, 'utf8');
const compiled = ts.transpileModule(source, {
  compilerOptions: {
    module: ts.ModuleKind.ES2022,
    target: ts.ScriptTarget.ES2020,
    verbatimModuleSyntax: true,
  },
}).outputText;

const tempModulePath = path.resolve('tmp/download-state.test-output.mjs');
fs.mkdirSync(path.dirname(tempModulePath), { recursive: true });
fs.writeFileSync(tempModulePath, compiled);

const {
  DOWNLOAD_HISTORY_LIMIT,
  archiveFinishedRows,
  createDownloadKey,
  deleteHistoryRow,
  hasActiveDownloadForFormat,
  mergeRowsById,
  pruneDownloadHistory,
  removeCurrentRow,
  replaceCurrentDownloadRow,
  updateDownloadProgress,
  updateDownloadStatus,
  upsertCurrentDownloadRow,
} = await import(pathToFileURL(tempModulePath).href);

assert.equal(createDownloadKey('https://x.com/u/status/1', 'best'), 'https://x.com/u/status/1::best');
assert.equal(createDownloadKey('https://x.com/u/status/1', null), 'https://x.com/u/status/1::');

const fallbackState = {
  current: [],
  history: [
    {
      id: 'task-1',
      title: '同一个视频',
      sessionLabel: '历史任务',
      status: 'queued',
      progress: '0%',
    },
  ],
};

const next = upsertCurrentDownloadRow(fallbackState, {
  id: 'task-1',
  title: '同一个视频',
  sessionLabel: 'x.com',
  status: 'queued',
  progress: '0%',
  sourceUrl: 'https://x.com/u/status/1',
  formatId: 'best',
});

assert.equal(next.current.length, 1);
assert.equal(next.history.length, 0);
assert.equal(next.current[0].id, 'task-1');
assert.equal(next.current[0].sessionLabel, 'x.com');

const replaced = upsertCurrentDownloadRow(
  {
    current: [
      {
        id: 'task-1',
        title: '同一个视频',
        sessionLabel: '旧标签',
        status: 'queued',
        progress: '0%',
      },
    ],
    history: [],
  },
  {
    id: 'task-1',
    title: '同一个视频',
    sessionLabel: '新标签',
    status: 'queued',
    progress: '0%',
  },
);

assert.equal(replaced.current.length, 1);
assert.equal(replaced.current[0].sessionLabel, '新标签');

const duplicateFormat = upsertCurrentDownloadRow(
  {
    current: [
      {
        id: 'task-old',
        title: '同一个视频',
        sessionLabel: 'x.com',
        status: 'downloading',
        progress: '20%',
        sourceUrl: 'https://x.com/u/status/1',
        formatId: 'best',
      },
    ],
    history: [],
  },
  {
    id: 'task-new',
    title: '同一个视频',
    sessionLabel: 'x.com',
    status: 'queued',
    progress: '0%',
    sourceUrl: 'https://x.com/u/status/1',
    formatId: 'best',
  },
);

assert.equal(duplicateFormat.current.length, 1);
assert.equal(duplicateFormat.current[0].id, 'task-new');
assert.equal(
  hasActiveDownloadForFormat(duplicateFormat, 'https://x.com/u/status/1', 'best'),
  true,
);

const replacedOptimistic = replaceCurrentDownloadRow(
  {
    current: [
      {
        id: 'pending:https://x.com/u/status/1::best',
        title: '正在加入',
        sessionLabel: 'x.com',
        status: 'starting',
        progress: '0%',
        sourceUrl: 'https://x.com/u/status/1',
        formatId: 'best',
      },
    ],
    history: [],
  },
  'pending:https://x.com/u/status/1::best',
  {
    id: 'task-2',
    title: '真实任务',
    sessionLabel: 'x.com',
    status: 'queued',
    progress: '0%',
    sourceUrl: 'https://x.com/u/status/1',
    formatId: 'best',
  },
);
assert.equal(replacedOptimistic.current.length, 1);
assert.equal(replacedOptimistic.current[0].id, 'task-2');
assert.equal(replacedOptimistic.current[0].title, '真实任务');

const merged = mergeRowsById(
  [
    { id: 'task-1', title: '最新', status: 'completed', progress: '100%' },
    { id: 'task-2', title: '另一个', status: 'failed', progress: '20%' },
  ],
  [
    { id: 'task-1', title: '旧的', status: 'queued', progress: '0%' },
    { id: 'task-3', title: '第三个', status: 'completed', progress: '100%' },
  ],
);
assert.deepEqual(
  merged.map((row) => row.id),
  ['task-1', 'task-2', 'task-3'],
);
assert.equal(merged[0].title, '最新');

const archived = archiveFinishedRows({
  current: [
    { id: 'active', title: '进行中', status: 'downloading', progress: '10%' },
    { id: 'done', title: '完成', status: 'completed', progress: '100%' },
    { id: 'failed', title: '失败', status: 'failed', progress: '30%' },
  ],
  history: [{ id: 'old', title: '旧历史', status: 'completed', progress: '100%' }],
});
assert.deepEqual(
  archived.current.map((row) => row.id),
  ['active'],
);
assert.deepEqual(
  archived.history.map((row) => row.id),
  ['done', 'failed', 'old'],
);

const statusUpdated = updateDownloadStatus(
  {
    current: [{ id: 'task-status', title: '旧标题', status: 'downloading', progress: '12%' }],
    history: [{ id: 'task-status', title: '旧历史', status: 'queued', progress: '0%' }],
  },
  {
    taskId: 'task-status',
    title: '新标题',
    status: 'completed',
    outputPath: 'D:/video.mp4',
  },
);
assert.equal(statusUpdated.current.length, 0);
assert.equal(statusUpdated.history.length, 1);
assert.equal(statusUpdated.history[0].title, '新标题');
assert.equal(statusUpdated.history[0].progress, '100%');
assert.equal(statusUpdated.history[0].outputPath, 'D:/video.mp4');

const progressUpdated = updateDownloadProgress(
  { current: [], history: [] },
  { taskId: 'task-progress', percent: '44%', speed: '1 MiB/s' },
);
assert.equal(progressUpdated.current[0].id, 'task-progress');
assert.equal(progressUpdated.current[0].status, 'downloading');
assert.equal(progressUpdated.current[0].progress, '44%');
assert.equal(progressUpdated.current[0].speed, '1 MiB/s');

const deletedHistory = deleteHistoryRow(
  {
    current: [],
    history: [
      { id: 'keep', title: '保留', status: 'completed', progress: '100%' },
      { id: 'delete', title: '删除', status: 'completed', progress: '100%' },
    ],
  },
  'delete',
);
assert.deepEqual(
  deletedHistory.history.map((row) => row.id),
  ['keep'],
);

const removedCurrent = removeCurrentRow(
  {
    current: [
      { id: 'remove', title: '移除', status: 'downloading', progress: '1%' },
      { id: 'stay', title: '保留', status: 'queued', progress: '0%' },
    ],
    history: [],
  },
  'remove',
);
assert.deepEqual(
  removedCurrent.current.map((row) => row.id),
  ['stay'],
);

const oversizedHistory = Array.from({ length: DOWNLOAD_HISTORY_LIMIT + 5 }, (_item, index) => ({
  id: `history-${index}`,
  title: `历史 ${index}`,
  status: 'completed',
  progress: '100%',
}));
const pruned = pruneDownloadHistory(oversizedHistory);
assert.equal(pruned.length, DOWNLOAD_HISTORY_LIMIT);
assert.equal(pruned[0].id, 'history-0');
assert.equal(pruned.at(-1).id, `history-${DOWNLOAD_HISTORY_LIMIT - 1}`);

fs.rmSync(tempModulePath, { force: true });
