import { sqlite } from '@flue/runtime/node';
import { ensureRuntimeDir, runtimeDbPath } from './shared/runtime-paths.ts';

const dbPath = runtimeDbPath();
ensureRuntimeDir(dbPath);

export default sqlite(dbPath);
