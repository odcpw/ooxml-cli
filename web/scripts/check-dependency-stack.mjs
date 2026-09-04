#!/usr/bin/env node
import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';

const lockPath = fileURLToPath(new URL('../package-lock.json', import.meta.url));
const lock = JSON.parse(await readFile(lockPath, 'utf8'));
const expected = new Map([
  ['@flue/cli', new Set(['1.0.0-beta.9'])],
  ['@flue/runtime', new Set(['1.0.0-beta.9'])],
  ['@flue/sdk', new Set(['1.0.0-beta.9'])],
  ['@cloudflare/vite-plugin', new Set(['1.54.4'])],
  ['wrangler', new Set(['4.129.0'])],
  ['miniflare', new Set(['5.20260903.0-alpha'])],
  ['esbuild', new Set(['0.28.1'])],
  ['undici', new Set(['7.29.0'])],
  ['ws', new Set(['8.21.0'])],
]);

const resolved = new Map([...expected.keys()].map((name) => [name, new Set()]));
for (const [path, entry] of Object.entries(lock.packages || {})) {
  const name = packageName(path);
  if (name && resolved.has(name) && entry.version) resolved.get(name).add(entry.version);
}

const failures = [];
for (const [name, expectedVersions] of expected) {
  const actualVersions = resolved.get(name);
  const unexpected = [...actualVersions].filter((version) => !expectedVersions.has(version));
  const missing = [...expectedVersions].filter((version) => !actualVersions.has(version));
  if (unexpected.length || missing.length) {
    failures.push({ name, expected: [...expectedVersions], actual: [...actualVersions] });
  }
}

if (failures.length) {
  process.stderr.write(`${JSON.stringify({ ok: false, failures }, null, 2)}\n`);
  process.exitCode = 1;
} else {
  console.log(
    JSON.stringify(
      {
        ok: true,
        packages: Object.fromEntries([...resolved].map(([name, versions]) => [name, [...versions]])),
      },
      null,
      2,
    ),
  );
}

function packageName(path) {
  if (!path.includes('node_modules/')) return undefined;
  const suffix = path.split('node_modules/').at(-1);
  const parts = suffix.split('/');
  return suffix.startsWith('@') ? parts.slice(0, 2).join('/') : parts[0];
}
