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

const { createDownloadKey, hasActiveDownloadForFormat, upsertCurrentDownloadRow } = await import(
  pathToFileURL(tempModulePath).href
);

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

fs.rmSync(tempModulePath, { force: true });
