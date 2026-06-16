import { appendFile, mkdir, readFile, writeFile } from 'node:fs/promises';
import { createHash, randomBytes, randomUUID, timingSafeEqual } from 'node:crypto';
import { dirname, join, resolve } from 'node:path';
import type { Context, MiddlewareHandler } from 'hono';
import { deleteCookie, getCookie, setCookie } from 'hono/cookie';
import { runtimeDataRoot } from './runtime-paths.ts';

export type AuthUser = {
  id: string;
  email: string;
  createdAt: string;
  lastSeenAt: string;
};

export type AuthEnv = {
  Variables: {
    authUser: AuthUser;
  };
};

type SessionRecord = {
  id: string;
  userId: string;
  tokenHash: string;
  createdAt: string;
  expiresAt: string;
  lastSeenAt: string;
};

type MagicLinkRecord = {
  id: string;
  email: string;
  tokenHash: string;
  createdAt: string;
  expiresAt: string;
};

type OAuthIdentityRecord = {
  provider: OAuthProvider;
  subject: string;
  userId: string;
  email: string;
  updatedAt: string;
};

type RateLimitRecord = {
  scope: string;
  bucketStart: number;
  count: number;
  updatedAt: string;
};

type AuthState = {
  users: AuthUser[];
  sessions: SessionRecord[];
  magicLinks: MagicLinkRecord[];
  oauthIdentities: OAuthIdentityRecord[];
  rateLimits: RateLimitRecord[];
};

export type OAuthProvider = 'google' | 'microsoft';

type OAuthStateCookie = {
  provider: OAuthProvider;
  state: string;
  codeVerifier: string;
  returnTo: string;
};

export const SESSION_COOKIE_NAME = 'ooxml_session';
export const CSRF_COOKIE_NAME = 'ooxml_csrf';
export const CSRF_HEADER_NAME = 'x-ooxml-csrf';

const sessionTtlMs = 30 * 24 * 60 * 60 * 1000;
const csrfTtlSeconds = 30 * 24 * 60 * 60;
const magicLinkTokenTtlMs = 15 * 60 * 1000;
const magicLinkActiveTokenLimit = 3;
const magicLinkRequestWindowMs = 60 * 60 * 1000;
const oauthStateTtlSeconds = 10 * 60;
const stateChangingMethods = new Set(['DELETE', 'PATCH', 'POST', 'PUT']);
const providers = new Set<OAuthProvider>(['google', 'microsoft']);

let authMutationQueue: Promise<unknown> = Promise.resolve();

export function getAuthUser(c: Context): AuthUser {
  const user = c.get('authUser');
  if (!user) throw new Error('Authentication required.');
  return user as AuthUser;
}

export function isStateChangingMethod(method: string): boolean {
  return stateChangingMethods.has(method.toUpperCase());
}

export async function currentSessionUser(c: Context): Promise<AuthUser | null> {
  const devUser = await maybeIssueDevBypassSession(c);
  if (devUser) return devUser;

  const cookieValue = getCookie(c, SESSION_COOKIE_NAME);
  if (!cookieValue) return null;
  return validateSessionCookie(cookieValue);
}

export const requireAuth: MiddlewareHandler<AuthEnv> = async (c, next) => {
  const user = await currentSessionUser(c);
  if (!user) {
    return unauthenticated(c);
  }

  c.set('authUser', user);

  if (isStateChangingMethod(c.req.method) && !hasValidCsrf(c)) {
    return c.json({ error: 'CSRF token required.' }, 403);
  }

  await next();
};

export function requireJsonContent(c: Context): Response | null {
  const contentType = c.req.header('content-type') ?? '';
  if (!contentType.toLowerCase().startsWith('application/json')) {
    return c.json({ error: 'Content-Type must be application/json.' }, 415);
  }
  return null;
}

export async function issueSessionForEmail(c: Context, email: string, identity?: { provider: OAuthProvider; subject: string }): Promise<AuthUser> {
  const normalizedEmail = normalizeEmail(email);
  assertValidEmail(normalizedEmail);
  assertEmailAllowed(normalizedEmail);

  const now = new Date();
  const token = randomToken();
  const tokenHash = sha256(token);
  const sessionId = randomUUID();
  const expiresAt = new Date(now.getTime() + sessionTtlMs).toISOString();

  const user = await mutateAuthState((state) => {
    cleanupAuthState(state, now);
    let existing = state.users.find((candidate) => candidate.email === normalizedEmail);
    if (!existing) {
      existing = {
        id: `user-${randomUUID()}`,
        email: normalizedEmail,
        createdAt: now.toISOString(),
        lastSeenAt: now.toISOString(),
      };
      state.users.push(existing);
    } else {
      existing.lastSeenAt = now.toISOString();
    }

    if (identity) {
      state.oauthIdentities = state.oauthIdentities.filter(
        (candidate) => !(candidate.provider === identity.provider && candidate.subject === identity.subject),
      );
      state.oauthIdentities.push({
        provider: identity.provider,
        subject: identity.subject,
        userId: existing.id,
        email: normalizedEmail,
        updatedAt: now.toISOString(),
      });
    }

    state.sessions.push({
      id: sessionId,
      userId: existing.id,
      tokenHash,
      createdAt: now.toISOString(),
      expiresAt,
      lastSeenAt: now.toISOString(),
    });
    pruneUserSessions(state, existing.id);
    return existing;
  });

  setSessionCookies(c, `${sessionId}.${token}`, Math.floor(sessionTtlMs / 1000));
  return user;
}

export function clearAuthCookies(c: Context): void {
  deleteCookie(c, SESSION_COOKIE_NAME, { path: '/' });
  deleteCookie(c, CSRF_COOKIE_NAME, { path: '/' });
}

export async function requestMagicLink(input: { c: Context; email: string }): Promise<{ ok: boolean; retryAfterSeconds?: number }> {
  const email = normalizeEmail(input.email);
  if (!isValidEmail(email)) {
    throw new Error('Enter a valid email address.');
  }

  const clientScope = clientIpScope(input.c);
  const emailScope = `magic-link:email:${sha256(email).slice(0, 32)}`;
  const globalScope = 'magic-link:global';
  const rateLimits = [
    await checkRateLimit(emailScope, 3, magicLinkRequestWindowMs),
    clientScope ? await checkRateLimit(`magic-link:${clientScope}`, 30, magicLinkRequestWindowMs) : { allowed: true as const },
    await checkRateLimit(globalScope, 300, magicLinkRequestWindowMs),
  ];
  const denied = rateLimits.find((candidate) => !candidate.allowed);
  if (denied && !denied.allowed) {
    return { ok: false, retryAfterSeconds: denied.retryAfterSeconds };
  }

  const now = new Date();
  const token = randomToken();
  const tokenHash = sha256(token);
  const expiresAt = new Date(now.getTime() + magicLinkTokenTtlMs);

  const created = await mutateAuthState((state) => {
    cleanupAuthState(state, now);
    const active = state.magicLinks.filter((link) => link.email === email && Date.parse(link.expiresAt) > now.getTime());
    if (active.length >= magicLinkActiveTokenLimit) return false;
    state.magicLinks.push({
      id: randomUUID(),
      email,
      tokenHash,
      createdAt: now.toISOString(),
      expiresAt: expiresAt.toISOString(),
    });
    return true;
  });

  if (!created) return { ok: true };

  try {
    await sendMagicLinkEmail({
      to: email,
      from: process.env.EMAIL_FROM || 'no-reply@ooxml-workbench.local',
      magicLinkUrl: buildMagicLinkUrl(baseUrlFor(input.c), token),
      expiresAt,
    });
  } catch (error) {
    await mutateAuthState((state) => {
      state.magicLinks = state.magicLinks.filter((link) => link.tokenHash !== tokenHash);
      return undefined;
    });
    console.warn('[ooxml-web] Magic-link delivery failed.', errorReason(error));
  }

  return { ok: true };
}

export async function consumeMagicLink(c: Context, token: string): Promise<AuthUser> {
  const tokenHash = sha256(token);
  const now = new Date();
  const result = await mutateAuthState((state) => {
    cleanupAuthState(state, now);
    const link = state.magicLinks.find((candidate) => timingSafeEqualString(candidate.tokenHash, tokenHash));
    if (!link) return null;
    if (Date.parse(link.expiresAt) <= now.getTime()) {
      state.magicLinks = state.magicLinks.filter((candidate) => candidate.id !== link.id);
      return null;
    }
    state.magicLinks = state.magicLinks.filter((candidate) => candidate.id !== link.id);
    return link.email;
  });

  if (!result) throw new Error('Magic link is invalid or expired.');
  return issueSessionForEmail(c, result);
}

export async function checkRateLimit(
  scope: string,
  limit: number,
  windowMs: number,
): Promise<{ allowed: true } | { allowed: false; retryAfterSeconds: number }> {
  const now = Date.now();
  const bucketStart = Math.floor(now / windowMs) * windowMs;
  return mutateAuthState((state) => {
    state.rateLimits = state.rateLimits.filter((bucket) => bucket.bucketStart >= now - 24 * 60 * 60 * 1000);
    let bucket = state.rateLimits.find((candidate) => candidate.scope === scope && candidate.bucketStart === bucketStart);
    if (!bucket) {
      bucket = { scope, bucketStart, count: 0, updatedAt: new Date(now).toISOString() };
      state.rateLimits.push(bucket);
    }
    bucket.count += 1;
    bucket.updatedAt = new Date(now).toISOString();
    if (bucket.count <= limit) return { allowed: true };
    return {
      allowed: false,
      retryAfterSeconds: Math.max(1, Math.ceil((bucketStart + windowMs - now) / 1000)),
    };
  });
}

export function rateLimitResponse(c: Context, retryAfterSeconds: number): Response {
  c.header('Retry-After', String(retryAfterSeconds));
  return c.json({ error: 'Too many requests. Try again later.' }, 429);
}

export function isOAuthProvider(value: string): value is OAuthProvider {
  return providers.has(value as OAuthProvider);
}

export async function startOAuth(c: Context, provider: OAuthProvider, returnTo?: string | null): Promise<Response> {
  const config = oauthProviderConfig(provider);
  const state = randomToken();
  const codeVerifier = randomToken(48);
  const requestUrl = new URL(c.req.url);
  const redirectUri = oauthRedirectUri(provider, requestUrl);
  const authorizationUrl = new URL(config.authorizationEndpoint);

  authorizationUrl.searchParams.set('client_id', config.clientId);
  authorizationUrl.searchParams.set('code_challenge', pkceChallenge(codeVerifier));
  authorizationUrl.searchParams.set('code_challenge_method', 'S256');
  authorizationUrl.searchParams.set('redirect_uri', redirectUri);
  authorizationUrl.searchParams.set('response_type', 'code');
  authorizationUrl.searchParams.set('scope', config.scopes.join(' '));
  authorizationUrl.searchParams.set('state', state);
  authorizationUrl.searchParams.set('prompt', 'select_account');

  setCookie(c, oauthStateCookieName(provider), encodeOAuthStateCookie({
    provider,
    state,
    codeVerifier,
    returnTo: getSafeRedirectPath(returnTo || '/'),
  }), {
    httpOnly: true,
    maxAge: oauthStateTtlSeconds,
    path: '/',
    sameSite: 'Lax',
    secure: secureCookies(),
  });

  return c.redirect(authorizationUrl.toString(), 303);
}

export async function finishOAuth(c: Context, provider: OAuthProvider): Promise<Response> {
  const cookieName = oauthStateCookieName(provider);
  const stateCookie = decodeOAuthStateCookie(getCookie(c, cookieName) ?? '');
  clearOAuthStateCookie(c, cookieName);

  const url = new URL(c.req.url);
  const state = url.searchParams.get('state') ?? '';
  const code = url.searchParams.get('code') ?? '';
  if (!stateCookie || stateCookie.provider !== provider || !timingSafeEqualString(stateCookie.state, state) || !code) {
    return c.redirect('/signin?error=oauth_state', 303);
  }

  try {
    const token = await exchangeOAuthCode({ provider, code, codeVerifier: stateCookie.codeVerifier, requestUrl: url });
    const userInfo = await fetchOAuthUserInfo(provider, token.accessToken);
    const email = extractVerifiedOAuthEmail(provider, userInfo, token.idTokenClaims);
    const subject = extractOAuthSubject(provider, userInfo, token.idTokenClaims);
    if (!email || !subject) {
      return c.redirect('/signin?error=oauth_email', 303);
    }
    await issueSessionForEmail(c, email, { provider, subject });
    return c.redirect(getSafeRedirectPath(stateCookie.returnTo), 303);
  } catch (error) {
    console.warn('[ooxml-web] OAuth sign-in failed.', { provider, reason: errorReason(error) });
    return c.redirect('/signin?error=oauth_failed', 303);
  }
}

export function getSafeRedirectPath(value: string | null | undefined): string {
  const fallback = '/';
  if (!value) return fallback;
  const trimmed = value.trim();
  if (!trimmed.startsWith('/')) return fallback;
  if (trimmed.startsWith('//')) return fallback;
  if (trimmed.includes('://') || trimmed.includes('\\')) return fallback;
  return trimmed;
}

export function hasAllowedVerificationOrigin(c: Context): boolean {
  const requestOrigin = new URL(c.req.url).origin;
  const allowedOrigins = new Set([requestOrigin]);
  const appBaseUrl = process.env.APP_BASE_URL?.trim();
  if (appBaseUrl) {
    try {
      allowedOrigins.add(new URL(appBaseUrl).origin);
    } catch {
      // Ignore invalid deployment hints; request origin remains authoritative.
    }
  }

  const origin = c.req.header('origin');
  if (origin) return allowedOrigins.has(origin);

  const referer = c.req.header('referer');
  if (!referer) return true;

  try {
    return allowedOrigins.has(new URL(referer).origin);
  } catch {
    return false;
  }
}

export function authUserResponse(user: AuthUser): Record<string, unknown> {
  return {
    id: user.id,
    email: user.email,
    createdAt: user.createdAt,
    lastSeenAt: user.lastSeenAt,
  };
}

async function validateSessionCookie(cookieValue: string): Promise<AuthUser | null> {
  const [sessionId, token] = cookieValue.split('.');
  if (!sessionId || !token) return null;
  const tokenHash = sha256(token);
  const now = new Date();

  return mutateAuthState((state) => {
    cleanupAuthState(state, now);
    const session = state.sessions.find(
      (candidate) => candidate.id === sessionId && timingSafeEqualString(candidate.tokenHash, tokenHash),
    );
    if (!session) return null;
    const user = state.users.find((candidate) => candidate.id === session.userId);
    if (!user) return null;
    session.expiresAt = new Date(now.getTime() + sessionTtlMs).toISOString();
    session.lastSeenAt = now.toISOString();
    user.lastSeenAt = now.toISOString();
    return user;
  });
}

async function maybeIssueDevBypassSession(c: Context): Promise<AuthUser | null> {
  if (process.env.OOXML_AUTH_DEV_BYPASS !== '1') return null;
  if (process.env.NODE_ENV === 'production') return null;
  const email = process.env.OOXML_DEV_AUTH_EMAIL || 'oliver@local.test';
  const existing = await validateSessionCookie(getCookie(c, SESSION_COOKIE_NAME) ?? '');
  if (existing?.email === normalizeEmail(email)) return existing;
  return issueSessionForEmail(c, email);
}

function setSessionCookies(c: Context, sessionCookieValue: string, maxAgeSeconds: number): void {
  setCookie(c, SESSION_COOKIE_NAME, sessionCookieValue, {
    httpOnly: true,
    maxAge: maxAgeSeconds,
    path: '/',
    sameSite: 'Lax',
    secure: secureCookies(),
  });
  setCookie(c, CSRF_COOKIE_NAME, randomUUID(), {
    httpOnly: false,
    maxAge: csrfTtlSeconds,
    path: '/',
    sameSite: 'Lax',
    secure: secureCookies(),
  });
}

function clearOAuthStateCookie(c: Context, cookieName: string): void {
  setCookie(c, cookieName, '', {
    httpOnly: true,
    maxAge: 0,
    path: '/',
    sameSite: 'Lax',
    secure: secureCookies(),
  });
}

function hasValidCsrf(c: Context): boolean {
  if (!hasAllowedVerificationOrigin(c)) return false;
  const csrfCookie = getCookie(c, CSRF_COOKIE_NAME);
  const csrfHeader = c.req.header(CSRF_HEADER_NAME);
  return Boolean(csrfCookie && csrfHeader && timingSafeEqualString(csrfCookie, csrfHeader));
}

function unauthenticated(c: Context): Response {
  const pathname = new URL(c.req.url).pathname;
  if (pathname.startsWith('/api/') || pathname.startsWith('/flue/')) {
    return c.json({ error: 'Authentication required.' }, 401);
  }
  const signin = new URL('/signin', c.req.url);
  signin.searchParams.set('returnTo', `${pathname}${new URL(c.req.url).search}`);
  return c.redirect(signin.toString(), 303);
}

async function mutateAuthState<T>(fn: (state: AuthState) => T | Promise<T>): Promise<T> {
  const run = authMutationQueue.then(async () => {
    const state = await loadAuthState();
    const result = await fn(state);
    await saveAuthState(state);
    return result;
  });
  authMutationQueue = run.then(
    () => undefined,
    () => undefined,
  );
  return run;
}

async function loadAuthState(): Promise<AuthState> {
  try {
    const raw = await readFile(authStatePath(), 'utf8');
    return normalizeAuthState(JSON.parse(raw) as Partial<AuthState>);
  } catch {
    return normalizeAuthState({});
  }
}

async function saveAuthState(state: AuthState): Promise<void> {
  const path = authStatePath();
  await mkdir(dirname(path), { recursive: true });
  await writeFile(path, `${JSON.stringify(normalizeAuthState(state), null, 2)}\n`, { mode: 0o600 });
}

function authStatePath(): string {
  return resolve(runtimeDataRoot(), 'auth', 'auth.json');
}

function normalizeAuthState(state: Partial<AuthState>): AuthState {
  return {
    users: Array.isArray(state.users) ? state.users : [],
    sessions: Array.isArray(state.sessions) ? state.sessions : [],
    magicLinks: Array.isArray(state.magicLinks) ? state.magicLinks : [],
    oauthIdentities: Array.isArray(state.oauthIdentities) ? state.oauthIdentities : [],
    rateLimits: Array.isArray(state.rateLimits) ? state.rateLimits : [],
  };
}

function cleanupAuthState(state: AuthState, now: Date): void {
  const nowMs = now.getTime();
  state.sessions = state.sessions.filter((session) => Date.parse(session.expiresAt) > nowMs);
  state.magicLinks = state.magicLinks.filter((link) => Date.parse(link.expiresAt) > nowMs);
}

function pruneUserSessions(state: AuthState, userId: string): void {
  const userSessions = state.sessions
    .filter((session) => session.userId === userId)
    .sort((a, b) => Date.parse(a.createdAt) - Date.parse(b.createdAt));
  while (userSessions.length > 10) {
    const oldest = userSessions.shift();
    if (!oldest) break;
    state.sessions = state.sessions.filter((session) => session.id !== oldest.id);
  }
}

type MagicLinkEmail = {
  to: string;
  from: string;
  magicLinkUrl: string;
  expiresAt: Date;
};

async function sendMagicLinkEmail(email: MagicLinkEmail): Promise<void> {
  const transport = (process.env.EMAIL_TRANSPORT || (process.env.NODE_ENV === 'production' ? 'postmark' : 'dev')).toLowerCase();
  if (transport === 'dev') {
    const logPath = resolve(process.env.OOXML_MAGIC_LINK_LOG || join(runtimeDataRoot(), 'auth', 'magic-links.jsonl'));
    await mkdir(dirname(logPath), { recursive: true });
    await appendFile(
      logPath,
      `${JSON.stringify({
        kind: 'magic-link',
        to: email.to,
        from: email.from,
        subject: 'Sign in to OOXML Workbench',
        magicLinkUrl: email.magicLinkUrl,
        expiresAt: email.expiresAt.toISOString(),
        sentAt: new Date().toISOString(),
      })}\n`,
      { encoding: 'utf8', mode: 0o600 },
    );
    return;
  }

  if (transport === 'resend') {
    const apiKey = requiredEnv('RESEND_API_KEY');
    const response = await fetch(process.env.RESEND_ENDPOINT || 'https://api.resend.com/emails', {
      method: 'POST',
      headers: {
        authorization: `Bearer ${apiKey}`,
        'content-type': 'application/json',
      },
      body: JSON.stringify({
        from: email.from,
        to: email.to,
        subject: 'Sign in to OOXML Workbench',
        text: magicLinkText(email),
        html: magicLinkHtml(email),
      }),
    });
    if (!response.ok) throw new Error(`Resend email send failed with status ${response.status}.`);
    return;
  }

  if (transport === 'postmark') {
    const response = await fetch(process.env.POSTMARK_ENDPOINT || 'https://api.postmarkapp.com/email', {
      method: 'POST',
      headers: {
        accept: 'application/json',
        'content-type': 'application/json',
        'x-postmark-server-token': requiredEnv('POSTMARK_SERVER_TOKEN'),
      },
      body: JSON.stringify({
        From: email.from,
        To: email.to,
        Subject: 'Sign in to OOXML Workbench',
        TextBody: magicLinkText(email),
        HtmlBody: magicLinkHtml(email),
        MessageStream: process.env.POSTMARK_MESSAGE_STREAM || undefined,
        TrackLinks: 'None',
        TrackOpens: false,
      }),
    });
    if (!response.ok) throw new Error(`Postmark email send failed with status ${response.status}.`);
    return;
  }

  if (transport === 'mailgun') {
    const body = new URLSearchParams({
      from: email.from,
      to: email.to,
      subject: 'Sign in to OOXML Workbench',
      text: magicLinkText(email),
      html: magicLinkHtml(email),
      'o:tracking': 'no',
      'o:tracking-clicks': 'no',
      'o:tracking-opens': 'no',
    });
    const domain = requiredEnv('MAILGUN_DOMAIN');
    const base = (process.env.MAILGUN_BASE_URL || 'https://api.mailgun.net').replace(/\/+$/, '');
    const response = await fetch(`${base}/v3/${encodeURIComponent(domain)}/messages`, {
      method: 'POST',
      headers: {
        authorization: `Basic ${Buffer.from(`api:${requiredEnv('MAILGUN_API_KEY')}`).toString('base64')}`,
        'content-type': 'application/x-www-form-urlencoded',
      },
      body,
    });
    if (!response.ok) throw new Error(`Mailgun email send failed with status ${response.status}.`);
    return;
  }

  throw new Error(`Unsupported EMAIL_TRANSPORT: ${transport}`);
}

function magicLinkText(email: MagicLinkEmail): string {
  return [
    'Sign in to OOXML Workbench:',
    email.magicLinkUrl,
    '',
    `This link expires at ${email.expiresAt.toISOString()}.`,
  ].join('\n');
}

function magicLinkHtml(email: MagicLinkEmail): string {
  const link = escapeHtml(email.magicLinkUrl);
  return [
    '<p>Sign in to OOXML Workbench:</p>',
    `<p><a href="${link}">${link}</a></p>`,
    `<p>This link expires at ${escapeHtml(email.expiresAt.toISOString())}.</p>`,
  ].join('');
}

type OAuthProviderConfig = {
  authorizationEndpoint: string;
  tokenEndpoint: string;
  userInfoEndpoint: string;
  clientId: string;
  clientSecret: string;
  scopes: string[];
};

function oauthProviderConfig(provider: OAuthProvider): OAuthProviderConfig {
  if (provider === 'microsoft') {
    const tenant = process.env.MICROSOFT_OAUTH_TENANT || process.env.AZURE_AD_TENANT_ID || 'common';
    return {
      authorizationEndpoint: `https://login.microsoftonline.com/${encodeURIComponent(tenant)}/oauth2/v2.0/authorize`,
      tokenEndpoint: `https://login.microsoftonline.com/${encodeURIComponent(tenant)}/oauth2/v2.0/token`,
      userInfoEndpoint: 'https://graph.microsoft.com/oidc/userinfo',
      clientId: requiredEnv('MICROSOFT_OAUTH_CLIENT_ID', 'AZURE_AD_CLIENT_ID'),
      clientSecret: requiredEnv('MICROSOFT_OAUTH_CLIENT_SECRET', 'AZURE_AD_CLIENT_SECRET'),
      scopes: ['openid', 'email', 'profile'],
    };
  }
  return {
    authorizationEndpoint: 'https://accounts.google.com/o/oauth2/v2/auth',
    tokenEndpoint: 'https://oauth2.googleapis.com/token',
    userInfoEndpoint: 'https://openidconnect.googleapis.com/v1/userinfo',
    clientId: requiredEnv('GOOGLE_OAUTH_CLIENT_ID', 'GOOGLE_CLIENT_ID'),
    clientSecret: requiredEnv('GOOGLE_OAUTH_CLIENT_SECRET', 'GOOGLE_CLIENT_SECRET'),
    scopes: ['openid', 'email', 'profile'],
  };
}

function oauthRedirectUri(provider: OAuthProvider, requestUrl: URL): string {
  const explicit =
    provider === 'microsoft'
      ? process.env.MICROSOFT_OAUTH_REDIRECT_URI || process.env.AZURE_AD_REDIRECT_URI
      : process.env.GOOGLE_OAUTH_REDIRECT_URI || process.env.GOOGLE_REDIRECT_URI;
  if (explicit?.trim()) return explicit.trim();
  return new URL(`/api/auth/oauth/${provider}/callback`, baseOrigin(requestUrl)).toString();
}

async function exchangeOAuthCode(input: {
  provider: OAuthProvider;
  code: string;
  codeVerifier: string;
  requestUrl: URL;
}): Promise<{ accessToken: string; idTokenClaims: Record<string, unknown> | null }> {
  const config = oauthProviderConfig(input.provider);
  const body = new URLSearchParams({
    client_id: config.clientId,
    client_secret: config.clientSecret,
    code: input.code,
    code_verifier: input.codeVerifier,
    grant_type: 'authorization_code',
    redirect_uri: oauthRedirectUri(input.provider, input.requestUrl),
    scope: config.scopes.join(' '),
  });
  const response = await fetch(config.tokenEndpoint, {
    method: 'POST',
    headers: {
      accept: 'application/json',
      'content-type': 'application/x-www-form-urlencoded',
    },
    body,
  });
  if (!response.ok) throw new Error(`${input.provider} OAuth token exchange failed with status ${response.status}.`);
  const payload = (await response.json()) as { access_token?: unknown; id_token?: unknown };
  if (typeof payload.access_token !== 'string' || !payload.access_token) {
    throw new Error(`${input.provider} OAuth token response did not include an access token.`);
  }
  return {
    accessToken: payload.access_token,
    idTokenClaims: typeof payload.id_token === 'string' ? decodeJwtPayload(payload.id_token) : null,
  };
}

async function fetchOAuthUserInfo(provider: OAuthProvider, accessToken: string): Promise<Record<string, unknown>> {
  const config = oauthProviderConfig(provider);
  const response = await fetch(config.userInfoEndpoint, {
    method: 'GET',
    headers: {
      accept: 'application/json',
      authorization: `Bearer ${accessToken}`,
    },
  });
  if (!response.ok) throw new Error(`${provider} OAuth userinfo request failed with status ${response.status}.`);
  return (await response.json()) as Record<string, unknown>;
}

function extractVerifiedOAuthEmail(
  provider: OAuthProvider,
  userInfo: Record<string, unknown>,
  idTokenClaims: Record<string, unknown> | null,
): string | null {
  const claims = idTokenClaims ?? userInfo;
  if (provider === 'google' && !isTruthy(claims.email_verified)) return null;
  if (provider === 'microsoft' && process.env.OOXML_MICROSOFT_REQUIRE_EDOV !== '0' && !isTruthy(claims.xms_edov)) return null;
  const email = normalizeEmail(typeof claims.email === 'string' ? claims.email : '');
  return isValidEmail(email) ? email : null;
}

function extractOAuthSubject(
  provider: OAuthProvider,
  userInfo: Record<string, unknown>,
  idTokenClaims: Record<string, unknown> | null,
): string | null {
  const claims = idTokenClaims ?? userInfo;
  if (typeof claims.sub === 'string' && claims.sub.trim()) return claims.sub.trim();
  if (provider === 'microsoft' && typeof claims.oid === 'string') {
    const tid = typeof claims.tid === 'string' ? claims.tid.trim() : '';
    return tid ? `${tid}:${claims.oid.trim()}` : claims.oid.trim();
  }
  return null;
}

function encodeOAuthStateCookie(value: OAuthStateCookie): string {
  return Buffer.from(JSON.stringify(value), 'utf8').toString('base64url');
}

function decodeOAuthStateCookie(value: string): OAuthStateCookie | null {
  try {
    const parsed = JSON.parse(Buffer.from(value, 'base64url').toString('utf8')) as Partial<OAuthStateCookie>;
    if (!parsed.provider || !isOAuthProvider(parsed.provider)) return null;
    if (typeof parsed.state !== 'string' || typeof parsed.codeVerifier !== 'string' || typeof parsed.returnTo !== 'string') {
      return null;
    }
    return {
      provider: parsed.provider,
      state: parsed.state,
      codeVerifier: parsed.codeVerifier,
      returnTo: getSafeRedirectPath(parsed.returnTo),
    };
  } catch {
    return null;
  }
}

function oauthStateCookieName(provider: OAuthProvider): string {
  return `ooxml_oauth_${provider}`;
}

function buildMagicLinkUrl(baseUrl: string, token: string): string {
  const url = new URL('/api/auth/magic-link/verify', baseUrl);
  url.searchParams.set('token', token);
  return url.toString();
}

function baseUrlFor(c: Context): string {
  return process.env.APP_BASE_URL?.trim() || baseOrigin(new URL(c.req.url));
}

function baseOrigin(requestUrl: URL): string {
  return process.env.APP_BASE_URL?.trim() || requestUrl.origin;
}

function normalizeEmail(email: string): string {
  return email.trim().toLowerCase();
}

function isValidEmail(email: string): boolean {
  return /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email);
}

function assertValidEmail(email: string): void {
  if (!isValidEmail(email)) throw new Error('Enter a valid email address.');
}

function assertEmailAllowed(email: string): void {
  const allowed = (process.env.OOXML_AUTH_ALLOWED_EMAIL_DOMAINS || '')
    .split(',')
    .map((domain) => domain.trim().toLowerCase())
    .filter(Boolean);
  if (!allowed.length) return;
  const domain = email.split('@')[1] ?? '';
  if (!allowed.includes(domain)) {
    throw new Error('This email domain is not allowed for this workbench.');
  }
}

function clientIpScope(c: Context): string | null {
  const raw =
    c.req.header('x-forwarded-for')?.split(',')[0]?.trim() ||
    c.req.header('cf-connecting-ip')?.trim() ||
    c.req.header('x-real-ip')?.trim() ||
    c.req.header('host')?.trim() ||
    '';
  if (!raw) return null;
  return `ip:${sha256(raw).slice(0, 32)}`;
}

function randomToken(bytes = 32): string {
  return randomBytes(bytes).toString('base64url');
}

function sha256(value: string): string {
  return createHash('sha256').update(value, 'utf8').digest('hex');
}

function pkceChallenge(codeVerifier: string): string {
  return createHash('sha256').update(codeVerifier).digest('base64url');
}

function timingSafeEqualString(a: string, b: string): boolean {
  const left = Buffer.from(a);
  const right = Buffer.from(b);
  if (left.length !== right.length) return false;
  return timingSafeEqual(left, right);
}

function secureCookies(): boolean {
  if (process.env.NODE_ENV === 'production') return true;
  const base = process.env.APP_BASE_URL?.trim();
  return Boolean(base?.startsWith('https://'));
}

function requiredEnv(primary: string, fallback?: string): string {
  const value = process.env[primary]?.trim() || (fallback ? process.env[fallback]?.trim() : '');
  if (!value) throw new Error(`${primary}${fallback ? ` or ${fallback}` : ''} is required.`);
  return value;
}

function isTruthy(value: unknown): boolean {
  return value === true || value === 'true';
}

function decodeJwtPayload(token: string): Record<string, unknown> | null {
  const [, payload] = token.split('.');
  if (!payload) return null;
  try {
    const decoded = JSON.parse(Buffer.from(payload, 'base64url').toString('utf8')) as unknown;
    return decoded && typeof decoded === 'object' && !Array.isArray(decoded) ? (decoded as Record<string, unknown>) : null;
  } catch {
    return null;
  }
}

function escapeHtml(value: string): string {
  return value.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}

function errorReason(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
