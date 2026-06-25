import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import ts from 'typescript';

const sourcePath = path.resolve('src/lib/async-pool.ts');
const source = fs.readFileSync(sourcePath, 'utf8');
const compiled = ts.transpileModule(source, {
  compilerOptions: {
    module: ts.ModuleKind.ES2022,
    target: ts.ScriptTarget.ES2020,
    verbatimModuleSyntax: true,
  },
}).outputText;

const tempModulePath = path.resolve('tmp/async-pool.test-output.mjs');
fs.mkdirSync(path.dirname(tempModulePath), { recursive: true });
fs.writeFileSync(tempModulePath, compiled);

const { mapWithConcurrency } = await import(pathToFileURL(tempModulePath).href);

function nextTick() {
  return new Promise((resolve) => setTimeout(resolve, 0));
}

const releaseQueue = [];
let running = 0;
let maxRunning = 0;
const startOrder = [];

const work = mapWithConcurrency([1, 2, 3, 4, 5], 2, async (value) => {
  running += 1;
  maxRunning = Math.max(maxRunning, running);
  startOrder.push(value);
  await new Promise((resolve) => releaseQueue.push(resolve));
  running -= 1;
  return value * 10;
});

await nextTick();
assert.deepEqual(startOrder, [1, 2]);
assert.equal(maxRunning, 2);

releaseQueue.shift()();
await nextTick();
assert.deepEqual(startOrder, [1, 2, 3]);
assert.equal(maxRunning, 2);

while (releaseQueue.length > 0) {
  releaseQueue.shift()();
  await nextTick();
}

const result = await work;
assert.deepEqual(result, [10, 20, 30, 40, 50]);
assert.equal(maxRunning, 2);

fs.rmSync(tempModulePath, { force: true });
