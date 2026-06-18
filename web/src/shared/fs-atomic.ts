import { rename, writeFile } from 'node:fs/promises';
import { randomUUID } from 'node:crypto';

/**
 * Write a file atomically: write to a sibling temp file, then rename over the
 * target. rename(2) is atomic on the same filesystem, so a crash or power loss
 * mid-write leaves either the old file or the new one intact — never a
 * truncated/corrupt file. Used for the small JSON state files (auth.json,
 * thread.json) whose corruption would otherwise log every user out or make a
 * thread unreadable.
 */
export async function atomicWriteFile(
  path: string,
  data: string | Uint8Array,
  options?: { mode?: number },
): Promise<void> {
  const tmp = `${path}.tmp-${randomUUID().slice(0, 8)}`;
  await writeFile(tmp, data, options);
  await rename(tmp, path);
}
