#!/usr/bin/env node
// SPEC #838: both runners publish the same ledger; a retry never changes CI's first result.
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { dirname } from 'node:path';

const args = {};
for (let i = 2; i < process.argv.length; i += 2) {
  const key = process.argv[i];
  if (!['--output', '--playwright', '--rust-first', '--rust-retry', '--date', '--run-url'].includes(key) || !process.argv[i + 1]) {
    throw new Error(`Unknown or missing argument: ${key}`);
  }
  args[key.slice(2)] = process.argv[i + 1];
}
if (!args.output || (!args.playwright && !args['rust-first'])) throw new Error('Specify --output and a runner report');
if (args['rust-retry'] && !args['rust-first']) throw new Error('--rust-retry requires --rust-first');
const date = args.date || new Date().toISOString().slice(0, 10);
const rows = [];
let playwrightFlaky = 0;
let rustFlaky = 0;
if (args.playwright) {
  const report = JSON.parse(readFileSync(args.playwright, 'utf8'));
  if (!Array.isArray(report.suites) || !Number.isFinite(Date.parse(report.stats?.startTime)) || !Number.isFinite(report.stats?.duration) || !['flaky', 'expected', 'unexpected', 'skipped'].every(key => Number.isInteger(report.stats?.[key]) && report.stats[key] >= 0)) throw new Error('Incomplete Playwright report');
  let testCount = 0;
  function walk(suites, parents = []) {
    for (const suite of suites) {
      const titles = [...parents, suite.title].filter(Boolean);
      walk(suite.suites || [], titles);
      for (const spec of suite.specs || []) {
        for (const test of spec.tests || []) {
          testCount++;
          if (test.status !== 'flaky') continue;
          playwrightFlaky++;
          rows.push([`Playwright: ${spec.file}:${spec.line || '?'} ${[...titles, spec.title].join(' > ')} (${test.projectName})`, date, 1, '未分類', '初回失敗・retry 成功（要調査）', args['run-url'] || 'local']);
        }
      }
    }
  }
  walk(report.suites);
  if (testCount !== ['flaky', 'expected', 'unexpected', 'skipped'].reduce((sum, key) => sum + report.stats[key], 0)) throw new Error('Incomplete Playwright test inventory');
  if (playwrightFlaky !== report.stats.flaky) throw new Error('Playwright flaky count does not match test records');
}
function rustResults(file) {
  const results = new Map();
  let harness;
  let completed = false;
  for (const raw of readFileSync(file, 'utf8').split(/\r?\n/)) {
    const line = raw.replace(/\x1b\[[0-9;]*m/g, '');
    const running = line.match(/^\s*Running (.+?)\s+\(/) || line.match(/^\s*Doc-tests\s+(.+)/);
    if (running) {
      if (harness && !completed) throw new Error(`Incomplete Rust harness: ${harness}`);
      harness = running[1];
      completed = false;
    }
    if (/^test result: (ok|FAILED)\./.test(line)) completed = true;
    const test = line.match(/^test (.+?) \.\.\. (ok|FAILED)\s*$/);
    if (test && harness) results.set(`${harness} :: ${test[1]}`, test[2]);
  }
  if (!harness || !completed) throw new Error(`Incomplete Rust report: ${file}`);
  return results;
}
if (args['rust-first']) {
  const first = rustResults(args['rust-first']);
  const retry = args['rust-retry'] ? rustResults(args['rust-retry']) : new Map();
  for (const [test, status] of first) {
    if (status === 'FAILED' && retry.get(test) === 'ok') {
      rustFlaky++;
      rows.push([`Rust: ${test}`, date, 1, '未分類', '初回失敗・診断再実行成功（要調査）', args['run-url'] || 'local']);
    }
  }
}
const escapeCell = value => String(value).replace(/\|/g, '\\|').replace(/[\r\n]/g, ' ');
const markdown = ['# フレーク観測台帳', '', 'CI の観測結果。確定した根因は docs/flaky-tests.md に統合する。初回失敗を成功へ置換しない。', '', '| spec | 観測日 | 発生回数 | 根因パターン | 根因 | 関連 Issue・PR / run |', '| --- | --- | --- | --- | --- | --- |', ...rows.map(row => `| ${row.map(escapeCell).join(' | ')} |`), ''].join('\n');
mkdirSync(dirname(args.output), { recursive: true });
writeFileSync(args.output, markdown);
const summary = { playwright_flaky: playwrightFlaky, rust_flaky: rustFlaky, observations: rows };
writeFileSync(args.output.replace(/\.md$/, '') + '.json', JSON.stringify(summary, null, 2) + '\n');
console.log(JSON.stringify(summary));
