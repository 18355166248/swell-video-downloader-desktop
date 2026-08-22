import assert from 'node:assert/strict';
import { mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import ts from 'typescript';

const sourcePath = resolve('src/lib/progress-buffer.ts');
const outputPath = resolve('tmp/progress-buffer.test-output.mjs');
mkdirSync(dirname(outputPath), { recursive: true });

const source = readFileSync(sourcePath, 'utf8');
const transpiled = ts.transpileModule(source, {
  compilerOptions: {
    module: ts.ModuleKind.ES2022,
    target: ts.ScriptTarget.ES2022,
    strict: true,
  },
});
writeFileSync(outputPath, transpiled.outputText);

const { createProgressBuffer } = await import(pathToFileURL(outputPath).href);

const tick = (taskId, percent) => ({ task_id: taskId, percent, speed: '1 MiB/s' });

// Many ticks for one task collapse into that task's latest position.
{
  const batches = [];
  const buffer = createProgressBuffer(5, (batch) => batches.push(batch));

  buffer.push(tick('task-1', '10%'));
  buffer.push(tick('task-1', '20%'));
  buffer.push(tick('task-2', '5%'));
  buffer.push(tick('task-1', '30%'));

  assert.deepEqual(batches, [], 'nothing is delivered before the window closes');

  buffer.flush();

  assert.equal(batches.length, 1, 'one window yields one batch');
  assert.deepEqual(
    batches[0].map((item) => [item.task_id, item.percent]),
    [
      ['task-1', '30%'],
      ['task-2', '5%'],
    ],
  );
}

// A dropped task is not delivered: its download finished, or its row was removed.
{
  const batches = [];
  const buffer = createProgressBuffer(5, (batch) => batches.push(batch));

  buffer.push(tick('task-1', '40%'));
  buffer.push(tick('task-2', '50%'));
  buffer.drop('task-1');
  buffer.flush();

  assert.deepEqual(
    batches[0].map((item) => item.task_id),
    ['task-2'],
  );
}

// Flushing an empty buffer is a no-op rather than an empty batch.
{
  const batches = [];
  const buffer = createProgressBuffer(5, (batch) => batches.push(batch));

  buffer.flush();
  buffer.push(tick('task-1', '60%'));
  buffer.cancel();
  buffer.flush();

  assert.deepEqual(batches, [], 'cancel discards what was queued');
}

// The timer delivers on its own once the window closes.
{
  const batches = [];
  const buffer = createProgressBuffer(10, (batch) => batches.push(batch));

  buffer.push(tick('task-1', '70%'));
  await new Promise((done) => setTimeout(done, 40));

  assert.equal(batches.length, 1, 'the scheduled flush fires without a manual flush');
  assert.equal(batches[0][0].percent, '70%');

  // A later tick starts a fresh window instead of riding the finished one.
  buffer.push(tick('task-1', '80%'));
  await new Promise((done) => setTimeout(done, 40));
  assert.equal(batches.length, 2);
  assert.equal(batches[1][0].percent, '80%');

  buffer.cancel();
}

rmSync(outputPath, { force: true });
