import assert from 'node:assert/strict';
import { randomBytes, createHash } from 'node:crypto';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';
import { Hono } from 'hono';
import { authMiddleware, privateAccessRoute, privateAccessHtml, requireAuthUser, logoutRoute } from '../src/shared/auth.ts';

test('private link creates a guarded session; rotation revokes link and session', async () => {
  const root = await mkdtemp(join(tmpdir(), 'ooxml-private-auth-'));
  const saved = { ...process.env };
  try {
    const secret = randomBytes(32).toString('base64url');
    process.env.OOXML_WEB_DATA_DIR = root;
    process.env.APP_BASE_URL = 'https://example.test/ooxml';
    process.env.NODE_ENV = 'production';
    process.env.OOXML_AUTH_ACCESS_ONLY = '1';
    process.env.OOXML_ACCESS_KEY_SHA256 = createHash('sha256').update(secret).digest('hex');
    const app = new Hono();
    app.post('/api/auth/access', privateAccessRoute);
    app.use('/api/*', authMiddleware);
    app.get('/api/threads', c => c.json({ owner: requireAuthUser(c).id }));
    app.post('/api/edit', c => c.json({ ok: true }));
    app.post('/api/logout', logoutRoute);
    const login = (token, origin = 'https://example.test') => app.request('https://example.test/api/auth/access', {
      method: 'POST', headers: { 'Content-Type': 'application/json', Origin: origin }, body: JSON.stringify({ token }),
    });
    assert.equal((await app.request('/api/threads')).status, 401);
    assert.equal((await login(secret, 'https://attacker.test')).status, 403);
    assert.equal((await login('')).status, 401);
    assert.equal((await login(randomBytes(32).toString('base64url'))).status, 401);
    const response = await login(secret);
    assert.equal(response.status, 200);
    assert.equal(response.headers.get('Cache-Control'), 'no-store');
    const cookies = response.headers.getSetCookie();
    assert.match(cookies.find(c => c.startsWith('ooxml_session=')), /HttpOnly/);
    assert.match(cookies.find(c => c.startsWith('ooxml_session=')), /Secure/);
    const cookie = cookies.map(c => c.split(';')[0]).join('; ');
    const csrf = cookies.find(c => c.startsWith('ooxml_csrf=')).split(';')[0].split('=')[1];
    const owner = await (await app.request('/api/threads', { headers: { Cookie: cookie } })).json();
    assert.match(owner.owner, /^user-/);
    assert.equal((await app.request('/api/edit', { method: 'POST', headers: { Cookie: cookie } })).status, 403);
    assert.equal((await app.request('/api/edit', { method: 'POST', headers: { Cookie: cookie, Origin: 'https://example.test', 'x-ooxml-csrf': csrf } })).status, 200);
    const repeated = await login(secret);
    assert.equal((await repeated.json()).user.id, owner.owner);
    process.env.OOXML_ACCESS_KEY_SHA256 = 'a'.repeat(64);
    assert.equal((await login(secret)).status, 401);
    assert.equal((await app.request('/api/threads', { headers: { Cookie: cookie } })).status, 401);
    delete process.env.OOXML_ACCESS_KEY_SHA256;
    assert.equal((await login(secret)).status, 404);
    const html = privateAccessHtml();
    assert.ok(html.includes('location.hash.slice(1)'));
    assert.ok(html.includes('history.replaceState'));
    assert.ok(html.includes('/ooxml/api/auth/access'));
    assert.ok(!html.includes(secret));
  } finally {
    for (const key of Object.keys(process.env)) if (!(key in saved)) delete process.env[key];
    Object.assign(process.env, saved);
    await rm(root, { recursive: true, force: true });
  }
});
