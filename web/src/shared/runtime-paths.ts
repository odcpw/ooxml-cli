import { dirname, resolve } from 'node:path';
import { mkdirSync } from 'node:fs';

const defaultDataDir = '../.flue-ooxml-web-data';

export function runtimeDataRoot(): string {
  return resolve(process.env.OOXML_WEB_DATA_DIR ?? defaultDataDir);
}

export function runtimeDbPath(): string {
  return resolve(runtimeDataRoot(), 'flue.db');
}

export function ensureRuntimeDir(path: string): void {
  mkdirSync(dirname(path), { recursive: true });
}
