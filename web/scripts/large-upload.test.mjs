import assert from 'node:assert/strict';
import test from 'node:test';
import { assertUploadSizes, uploadLimits, withUploadSlot, previewRequiresConfirmation } from '../src/shared/upload-limits.ts';

test('private uploads accept a 28 MB template and training decks of hundreds of MB', () => {
  const keys = ['OOXML_UPLOAD_MAX_BYTES', 'OOXML_UPLOAD_MAX_TOTAL_BYTES', 'OOXML_UPLOAD_MAX_FILES', 'OOXML_UPLOAD_MAX_UNCOMPRESSED_BYTES'];
  const previous = keys.map(key => process.env[key]);
  try {
    keys.forEach(key => delete process.env[key]);
    const MiB = 1024 * 1024;
    assert.doesNotThrow(() => assertUploadSizes([{ size: 28 * MiB }]));
    assert.doesNotThrow(() => assertUploadSizes([{ size: 700 * MiB }, { size: 28 * MiB }]));
    assert.doesNotThrow(() => assertUploadSizes([{ size: 1024 * MiB }]));
    assert.throws(() => assertUploadSizes([{ size: 1024 * MiB + 1 }]), /Each file/);
    assert.throws(() => assertUploadSizes([{ size: 700 * MiB }, { size: 700 * MiB }]), /separately/);
    assert.equal(uploadLimits().maxUncompressedBytes, 2048 * MiB);
    process.env.OOXML_UPLOAD_MAX_BYTES = String(50 * MiB);
    assert.throws(() => assertUploadSizes([{ size: 51 * MiB }]), /50 MB/);
    assert.equal(previewRequiresConfirmation(28 * MiB), false);
    assert.equal(previewRequiresConfirmation(300 * MiB), true);
  } finally {
    keys.forEach((key, i) => previous[i] === undefined ? delete process.env[key] : process.env[key] = previous[i]);
  }
});

test('upload decoding is serialized and a failed upload does not block the next user', async () => {
  let release;
  const gate = new Promise(resolve => { release = resolve; });
  const order = [];
  const first = withUploadSlot(async () => { order.push('first'); await gate; throw Error('bad upload'); });
  const failed = assert.rejects(first, /bad upload/);
  const second = withUploadSlot(async () => { order.push('second'); return 'saved'; });
  await Promise.resolve();
  assert.deepEqual(order, ['first']);
  release();
  await failed;
  assert.equal(await second, 'saved');
  assert.deepEqual(order, ['first', 'second']);
});
