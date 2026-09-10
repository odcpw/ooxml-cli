const MiB = 1024 * 1024;
function positiveLimit(name: string, fallback: number): number {
  const value = Number(process.env[name]);
  return Number.isSafeInteger(value) && value > 0 ? value : fallback;
}
export function uploadLimits() {
  return {
    maxFileBytes: positiveLimit('OOXML_UPLOAD_MAX_BYTES', 1024 * MiB),
    maxBatchBytes: positiveLimit('OOXML_UPLOAD_MAX_TOTAL_BYTES', 1024 * MiB),
    maxFiles: positiveLimit('OOXML_UPLOAD_MAX_FILES', 8),
    maxUncompressedBytes: positiveLimit('OOXML_UPLOAD_MAX_UNCOMPRESSED_BYTES', 2048 * MiB),
  };
}
export function assertUploadSizes(files: Array<{ size: number }>): void {
  const limits = uploadLimits();
  if (files.length > limits.maxFiles) throw new Error(`Upload at most ${limits.maxFiles} files at once.`);
  if (files.some(file => file.size > limits.maxFileBytes)) throw new Error(`Each file can be up to ${limits.maxFileBytes / MiB} MB.`);
  if (files.reduce((sum, file) => sum + file.size, 0) > limits.maxBatchBytes) {
    throw new Error(`Upload up to ${limits.maxBatchBytes / MiB} MB at a time. Please send these files separately.`);
  }
}
// Multipart decoding can hold several copies of a large file in memory. Let
// Node's request stream apply backpressure while another upload is being saved.
let uploadQueue: Promise<void> = Promise.resolve();
export async function withUploadSlot<T>(operation: () => Promise<T>): Promise<T> {
  const result = uploadQueue.then(operation);
  uploadQueue = result.then(() => undefined, () => undefined);
  return result;
}
export function previewRequiresConfirmation(sizeBytes = 0): boolean {
  return sizeBytes > 100 * MiB;
}
export function commandTimeoutMs(): number {
  return Math.min(30 * 60_000, positiveLimit('OOXML_COMMAND_TIMEOUT_MS', 600_000));
}
