import assert from 'node:assert/strict';
import { mkdirSync, readFileSync, rmSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { writeFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';
import ts from 'typescript';

const sourcePath = resolve('src/lib/bounded-set.ts');
const outputPath = resolve('tmp/bounded-set.test-output.mjs');
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

const { addBounded } = await import(pathToFileURL(outputPath).href);

// Under the limit, nothing is evicted.
{
  const set = new Set();
  addBounded(set, 'task-1', 3);
  addBounded(set, 'task-2', 3);
  addBounded(set, 'task-3', 3);

  assert.deepEqual([...set], ['task-1', 'task-2', 'task-3']);
}

// Past the limit, the oldest ids go first and the newest are always kept.
{
  const set = new Set();
  for (let index = 0; index < 10; index += 1) {
    addBounded(set, `task-${index}`, 3);
  }

  assert.equal(set.size, 3);
  assert.deepEqual([...set], ['task-7', 'task-8', 'task-9']);
  assert.ok(set.has('task-9'), 'the newest id must survive');
  assert.ok(!set.has('task-0'), 'the oldest id is evicted');
}

// Re-adding an id already present is a no-op: it neither duplicates nor evicts.
{
  const set = new Set();
  addBounded(set, 'task-1', 2);
  addBounded(set, 'task-2', 2);
  addBounded(set, 'task-1', 2);

  assert.equal(set.size, 2);
  assert.deepEqual([...set], ['task-1', 'task-2']);
}

// A non-positive limit keeps nothing rather than looping forever.
{
  const set = new Set(['old']);
  addBounded(set, 'task-1', 0);

  assert.equal(set.size, 0);
}

rmSync(outputPath, { force: true });
