import { createHash, randomBytes, randomUUID } from 'node:crypto';
import { appendFile, mkdir, readFile, writeFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import type { Context, MiddlewareHandler } from 'hono';
import { deleteCookie, getCookie, setCookie } from 'hono/cookie';
import { appAbsoluteUrl, withAppBasePath } from './app-url.ts';
import { dataRoot } from './storage.ts';
import { themeCss } from './theme.ts';

export const SESSION_COOKIE_NAME = 'ooxml_session';
export const CSRF_COOKIE_NAME = 'ooxml_csrf';
export const CSRF_HEADER_NAME = 'x-ooxml-csrf';

const sessionTtlMs = 30 * 24 * 60 * 60 * 1000;
const magicLinkTtlMs = 15 * 60 * 1000;
const oauthStateTtlSeconds = 10 * 60;
const csrfCookieMaxAgeSeconds = 30 * 24 * 60 * 60;
const maxActiveMagicTokens = 3;
const magicLinkRateLimitWindowMs = 60 * 60 * 1000;

type OAuthProvider = 'google' | 'microsoft';

export type AuthUser = {
  id: string;
  email: string;
  createdAt: string;
  updatedAt: string;
};

type SessionRecord = {
  id: string;
  tokenHash: string;
  userId: string;
  csrfToken: string;
  createdAt: string;
  expiresAt: string;
  lastSeenAt: string;
};

type MagicLinkRecord = {
  id: string;
  email: string;
  tokenHash: string;
  returnTo: string;
  createdAt: string;
  expiresAt: string;
};

type OAuthIdentityRecord = {
  provider: OAuthProvider;
  subject: string;
  userId: string;
  email: string;
  createdAt: string;
};

type RateLimitRecord = {
  key: string;
  bucketStart: string;
  count: number;
};

type AuthState = {
  users: AuthUser[];
  sessions: SessionRecord[];
  magicLinks: MagicLinkRecord[];
  oauthIdentities: OAuthIdentityRecord[];
  rateLimits: RateLimitRecord[];
};

type OAuthStateCookie = {
  codeVerifier: string;
  provider: OAuthProvider;
  returnTo: string;
  state: string;
};

export type AuthContext = {
  user: AuthUser;
  session: SessionRecord;
};

export type AuthEnv = {
  Variables: {
    auth?: AuthContext;
  };
};

type AppContext = Context<{ Variables: { auth?: AuthContext } }>;

let stateQueue: Promise<unknown> = Promise.resolve();

export const authMiddleware: MiddlewareHandler = async (c, next) => {
  const pathname = new URL(c.req.url).pathname;
  if (isPublicPath(pathname)) {
    await next();
    return;
  }

  const auth = await validateAuthContext(c as AppContext);
  if (!auth) {
    return unauthenticatedResponse(c);
  }

  refreshCsrfCookie(c, auth.session.csrfToken);

  if (isStateChangingMethod(c.req.method) && !hasValidCsrf(c, auth)) {
    return c.json({ error: 'CSRF token required.' }, 403);
  }

  c.set('auth', auth);
  await next();
};

export function requireAuth(c: AppContext): AuthContext {
  const auth = c.get('auth');
  if (!auth) {
    throw new Error('Authentication required.');
  }
  return auth;
}

export function requireAuthUser(c: AppContext): AuthUser {
  return requireAuth(c).user;
}

export function authUserResponse(user: AuthUser): { id: string; email: string } {
  return {
    id: user.id,
    email: user.email,
  };
}

export async function checkRateLimit(
  key: string,
  limit: number,
  windowMs: number,
): Promise<{ allowed: true } | { allowed: false; retryAfterSeconds: number }> {
  const now = new Date();
  const normalizedLimit = Number.isFinite(limit) && limit > 0 ? Math.trunc(limit) : 60;
  const normalizedWindowMs = Number.isFinite(windowMs) && windowMs > 0 ? Math.trunc(windowMs) : 60_000;
  return mutateAuthState(async (state) => {
    const bucketStart = new Date(Math.floor(now.getTime() / normalizedWindowMs) * normalizedWindowMs);
    state.rateLimits = state.rateLimits.filter((record) => now.getTime() - Date.parse(record.bucketStart) < 24 * 60 * 60 * 1000);
    let record = state.rateLimits.find((candidate) => candidate.key === key && candidate.bucketStart === bucketStart.toISOString());
    if (!record) {
      record = { key, bucketStart: bucketStart.toISOString(), count: 0 };
      state.rateLimits.push(record);
    }
    record.count += 1;
    if (record.count > normalizedLimit) {
      return {
        allowed: false,
        retryAfterSeconds: Math.max(1, Math.ceil((bucketStart.getTime() + normalizedWindowMs - now.getTime()) / 1000)),
      };
    }
    return { allowed: true };
  });
}

export function rateLimitResponse(c: Context, retryAfterSeconds: number): Response {
  return c.json(
    { error: 'Too many requests. Try again later.' },
    429,
    { 'Retry-After': String(Math.max(1, Math.ceil(retryAfterSeconds))) },
  );
}

export function signInHtml(input: { returnTo?: string | null } = {}): string {
  const returnTo = safeReturnTo(input.returnTo);
  const microsoftConfigured = isOAuthConfigured('microsoft');
  const googleConfigured = isOAuthConfigured('google');
  const devEnabled = isDevSessionEnabled();
  return `<!doctype html>
<html lang="en" class="dark">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>Sign in - OOXML Workbench</title>
    <style>
${themeCss()}
      /* Sign-in card layer */
      body {
        min-height: 100vh;
        display: grid;
        place-items: center;
        padding: var(--space-4);
      }
      main {
        width: min(420px, calc(100vw - 2 * var(--space-4)));
        border: var(--border);
        border-radius: var(--radius-lg);
        background: var(--color-surface);
        box-shadow: var(--shadow-sm);
        padding: var(--space-6);
      }
      h1 { margin: 0 0 var(--space-2); font-size: var(--text-xl); font-weight: var(--font-weight-semibold); letter-spacing: var(--tracking-tight); }
      p { margin: 0 0 var(--space-5); color: var(--color-muted); line-height: var(--leading-snug); }
      form, .stack { display: grid; gap: var(--space-3); }
      /* Email field — .ss-input */
      input[type="email"] {
        display: block; width: 100%;
        border: var(--border-width) solid var(--color-border); border-radius: var(--radius-md);
        background: var(--color-surface); color: var(--color-text);
        font-family: var(--font-sans); font-size: var(--text-sm);
        padding: 0 0.75rem; min-height: var(--control-h-md); box-shadow: var(--shadow-sm); outline: none;
        transition: border-color var(--duration-fast) var(--ease-standard), box-shadow var(--duration-fast) var(--ease-standard);
      }
      input[type="email"]::placeholder { color: var(--color-muted); }
      input[type="email"]:hover { border-color: var(--color-accent); }
      input[type="email"]:focus { border-color: var(--color-accent); box-shadow: 0 0 0 var(--ring-width) color-mix(in srgb, var(--color-accent) 35%, transparent); }
      /* Buttons / OAuth links — .ss-btn */
      button, a.button {
        display: inline-flex; align-items: center; justify-content: center; gap: var(--space-2);
        min-height: var(--control-h-md); padding: 0 0.75rem;
        border: var(--border-width) solid var(--color-accent); border-radius: var(--radius-md);
        background: var(--color-accent); color: var(--color-bg);
        font-family: var(--font-sans); font-size: var(--text-sm); font-weight: var(--font-weight-medium); line-height: 1;
        text-decoration: none; white-space: nowrap; cursor: pointer; outline: none;
        transition: background-color var(--duration-fast) var(--ease-standard), border-color var(--duration-fast) var(--ease-standard), opacity var(--duration-fast) var(--ease-standard);
      }
      button:hover, a.button:hover { opacity: 0.9; }
      a.button.secondary, button.secondary {
        background: var(--color-surface); border-color: var(--color-border); color: var(--color-text); opacity: 1;
      }
      a.button.secondary:hover, button.secondary:hover {
        opacity: 1; border-color: var(--color-accent); background: var(--color-surface-elev);
      }
      .divider { margin: var(--space-5) 0; border-top: var(--border); }
      .message { margin-top: var(--space-3); color: var(--color-muted); min-height: 18px; font-size: var(--text-sm); }
      .legal { display: flex; gap: var(--space-3); margin-top: var(--space-4); font-size: var(--text-sm); }
      .legal a { color: var(--color-muted); }
      .legal a:hover { color: var(--color-text); }
    </style>
  </head>
  <body>
    <main>
      <h1>OOXML Workbench</h1>
      <p>Sign in to use your private file library and agent threads.</p>
      <div class="stack">
        ${microsoftConfigured ? `<a class="button" href="${escapeHtml(withAppBasePath(`/api/auth/oauth/microsoft/start?returnTo=${encodeURIComponent(returnTo)}`))}">Continue with Microsoft</a>` : ''}
        ${googleConfigured ? `<a class="button secondary" href="${escapeHtml(withAppBasePath(`/api/auth/oauth/google/start?returnTo=${encodeURIComponent(returnTo)}`))}">Continue with Google</a>` : ''}
      </div>
      <div class="divider"></div>
      <form id="magicForm">
        <input id="email" name="email" type="email" autocomplete="email" placeholder="name@company.com" required />
        <input name="returnTo" type="hidden" value="${escapeHtml(returnTo)}" />
        <button type="submit">Send sign-in link</button>
      </form>
      ${devEnabled ? `<form method="post" action="${escapeHtml(withAppBasePath('/api/auth/dev-session'))}" style="margin-top:var(--space-2)"><input name="returnTo" type="hidden" value="${escapeHtml(returnTo)}" /><button class="secondary" type="submit">Use development session</button></form>` : ''}
      <div id="message" class="message"></div>
      <div class="legal"><a href="${escapeHtml(withAppBasePath('/about'))}">About</a><a href="${escapeHtml(withAppBasePath('/privacy'))}">Privacy</a><a href="${escapeHtml(withAppBasePath('/terms'))}">Terms</a></div>
    </main>
    <script>
      const form = document.getElementById('magicForm');
      const message = document.getElementById('message');
      form.addEventListener('submit', async (event) => {
        event.preventDefault();
        message.textContent = 'Sending sign-in link...';
        const response = await fetch(${JSON.stringify(withAppBasePath('/api/auth/magic-link/request'))}, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            email: document.getElementById('email').value,
            returnTo: ${JSON.stringify(returnTo)}
          })
        });
        const data = await response.json().catch(() => ({}));
        message.textContent = data.message || (response.ok ? 'Check your email.' : 'Sign-in link could not be requested.');
      });
    </script>
  </body>
</html>`;
}

export async function currentUserResponse(c: AppContext): Promise<Response> {
  const auth = requireAuth(c);
  return c.json({
    user: {
      id: auth.user.id,
      email: auth.user.email,
    },
    csrfHeader: CSRF_HEADER_NAME,
    csrfToken: auth.session.csrfToken,
  });
}

export async function requestMagicLinkRoute(c: Context): Promise<Response> {
  if (!hasAllowedOrigin(c)) {
    return c.json({ error: 'Sign-in request rejected.' }, 403);
  }
  const input = await readJsonOrForm(c);
  const email = normalizeEmail(stringValue(input.email));
  if (!isValidEmail(email)) {
    return c.json({ message: 'Enter a valid email address.' }, 400);
  }

  const rateLimit = await checkMagicLinkRateLimit(email, clientIp(c.req.raw.headers));
  if (!rateLimit.allowed) {
    return c.json(
      { message: 'Too many sign-in links requested. Try again later.' },
      429,
      { 'Retry-After': String(rateLimit.retryAfterSeconds) },
    );
  }

  const token = randomToken();
  const returnTo = safeReturnTo(stringValue(input.returnTo));
  const now = new Date();
  await mutateAuthState(async (state) => {
    pruneExpired(state, now);
    const active = state.magicLinks
      .filter((candidate) => candidate.email === email && Date.parse(candidate.expiresAt) > now.getTime())
      .sort((a, b) => Date.parse(a.createdAt) - Date.parse(b.createdAt));
    while (active.length >= maxActiveMagicTokens) {
      const oldest = active.shift();
      if (!oldest) break;
      state.magicLinks = state.magicLinks.filter((candidate) => candidate.id !== oldest.id);
    }
    state.magicLinks.push({
      id: randomUUID(),
      email,
      tokenHash: hashToken(token),
      returnTo,
      createdAt: now.toISOString(),
      expiresAt: new Date(now.getTime() + magicLinkTtlMs).toISOString(),
    });
  });

  await sendMagicLink({
    email,
    magicLinkUrl: appAbsoluteUrl(c, `/api/auth/magic-link/verify?token=${encodeURIComponent(token)}`),
    expiresAt: new Date(now.getTime() + magicLinkTtlMs),
  });

  return c.json({ message: 'Check your email for a sign-in link.' }, 202);
}

export function confirmMagicLinkHtml(token: string): string {
  return `<!doctype html>
<html lang="en" class="dark">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Sign in - OOXML Workbench</title>
  <style>
${themeCss()}
    /* Confirm card layer */
    body { min-height: 100vh; display: grid; place-items: center; padding: var(--space-4); }
    main {
      width: min(420px, calc(100vw - 2 * var(--space-4)));
      border: var(--border);
      border-radius: var(--radius-lg);
      background: var(--color-surface);
      box-shadow: var(--shadow-sm);
      padding: var(--space-6);
      text-align: center;
    }
    h1 { margin: 0 0 var(--space-2); font-size: var(--text-xl); font-weight: var(--font-weight-semibold); letter-spacing: var(--tracking-tight); }
    p { margin: 0 0 var(--space-5); color: var(--color-muted); line-height: var(--leading-snug); }
    button {
      display: inline-flex; align-items: center; justify-content: center; gap: var(--space-2);
      min-height: var(--control-h-md); padding: 0 1rem;
      border: var(--border-width) solid var(--color-accent); border-radius: var(--radius-md);
      background: var(--color-accent); color: var(--color-bg);
      font-family: var(--font-sans); font-size: var(--text-sm); font-weight: var(--font-weight-medium); line-height: 1;
      cursor: pointer; outline: none;
      transition: opacity var(--duration-fast) var(--ease-standard);
    }
    button:hover { opacity: 0.9; }
  </style>
</head>
<body>
  <main>
    <h1>Sign in to OOXML Workbench</h1>
    <p>Confirm this browser should be signed in.</p>
    <form method="post" action="${escapeHtml(withAppBasePath('/api/auth/magic-link/verify'))}">
      <input type="hidden" name="token" value="${escapeHtml(token)}" />
      <button type="submit">Sign in</button>
    </form>
  </main>
</body>
</html>`;
}

export async function verifyMagicLinkRoute(c: Context): Promise<Response> {
  if (!hasAllowedVerificationOrigin(c)) {
    return c.json({ message: 'Magic link is invalid or already used.' }, 403);
  }

  const input = await readJsonOrForm(c);
  const token = stringValue(input.token) || new URL(c.req.url).searchParams.get('token') || '';
  if (!token) {
    return c.json({ message: 'Magic link is invalid or already used.' }, 400);
  }

  const consumed = await mutateAuthState(async (state) => {
    const now = new Date();
    pruneExpired(state, now);
    const tokenHash = hashToken(token);
    const index = state.magicLinks.findIndex((candidate) => candidate.tokenHash === tokenHash);
    if (index === -1) return null;
    const record = state.magicLinks[index];
    if (!record || Date.parse(record.expiresAt) <= now.getTime()) {
      if (index !== -1) state.magicLinks.splice(index, 1);
      return { error: 'expired' as const };
    }
    state.magicLinks.splice(index, 1);
    const user = findOrCreateUserByEmail(state, record.email, now);
    return { user, returnTo: record.returnTo };
  });

  if (!consumed) {
    return c.json({ message: 'Magic link is invalid or already used.' }, 400);
  }
  if ('error' in consumed) {
    return c.json({ message: 'Magic link expired. Request a new sign-in link.' }, 410);
  }

  await issueSession(c, consumed.user);
  if (wantsHtml(c)) {
    return c.redirect(consumed.returnTo, 303);
  }
  return c.json({
    message: 'Signed in.',
    user: {
      id: consumed.user.id,
      email: consumed.user.email,
    },
  });
}

export async function startOAuthRoute(c: Context, provider: string): Promise<Response> {
  if (!isOAuthProvider(provider)) {
    return c.json({ message: 'Unsupported OAuth provider.' }, 404);
  }
  const config = oauthProviderConfig(provider);
  if (!config.clientId || !config.clientSecret) {
    return c.json({ message: `${provider} OAuth is not configured.` }, 404);
  }

  const state = randomToken();
  const codeVerifier = randomToken(48);
  const returnTo = safeReturnTo(new URL(c.req.url).searchParams.get('returnTo'));
  const redirectUri = oauthRedirectUri(c, provider);
  const authorizationUrl = new URL(config.authorizationEndpoint);
  authorizationUrl.searchParams.set('client_id', config.clientId);
  authorizationUrl.searchParams.set('code_challenge', pkceChallenge(codeVerifier));
  authorizationUrl.searchParams.set('code_challenge_method', 'S256');
  authorizationUrl.searchParams.set('redirect_uri', redirectUri);
  authorizationUrl.searchParams.set('response_type', 'code');
  authorizationUrl.searchParams.set('scope', config.scopes.join(' '));
  authorizationUrl.searchParams.set('state', state);
  authorizationUrl.searchParams.set('prompt', 'select_account');

  setCookie(c, oauthStateCookieName(provider), encodeOAuthStateCookie({ codeVerifier, provider, returnTo, state }), {
    httpOnly: true,
    maxAge: oauthStateTtlSeconds,
    path: '/',
    sameSite: 'Lax',
    secure: isSecureCookie(),
  });

  return c.redirect(authorizationUrl.toString(), 303);
}

export async function oauthCallbackRoute(c: Context, provider: string): Promise<Response> {
  if (!isOAuthProvider(provider)) {
    return c.json({ message: 'Unsupported OAuth provider.' }, 404);
  }

  const cookieName = oauthStateCookieName(provider);
  const stateCookie = decodeOAuthStateCookie(getCookie(c, cookieName) || '');
  const url = new URL(c.req.url);
  const stateParam = url.searchParams.get('state') || '';
  const code = url.searchParams.get('code') || '';
  const providerError = url.searchParams.get('error');

  deleteCookie(c, cookieName, { path: '/' });

  if (!stateCookie || stateCookie.provider !== provider || stateCookie.state !== stateParam || providerError || !code) {
    return c.redirect(withAppBasePath(`/signin?oauth=${encodeURIComponent(providerError || 'oauth_failed')}`), 303);
  }

  try {
    const token = await exchangeOAuthCode(c, provider, code, stateCookie.codeVerifier);
    const claims = await oauthIdentityClaims(provider, token);
    const email = extractVerifiedOAuthEmail(provider, claims);
    const subject = extractOAuthSubject(provider, claims);
    if (!email || !subject) {
      return c.redirect(withAppBasePath('/signin?oauth=oauth_email'), 303);
    }
    const user = await mutateAuthState(async (state) => {
      const now = new Date();
      const user = findOrCreateUserByEmail(state, email, now);
      if (!state.oauthIdentities.some((identity) => identity.provider === provider && identity.subject === subject)) {
        state.oauthIdentities.push({
          provider,
          subject,
          userId: user.id,
          email,
          createdAt: now.toISOString(),
        });
      }
      return user;
    });
    await issueSession(c, user);
    return c.redirect(stateCookie.returnTo, 303);
  } catch (error) {
    console.warn('OAuth sign-in failed.', { provider, reason: error instanceof Error ? error.message : String(error) });
    return c.redirect(withAppBasePath('/signin?oauth=oauth_failed'), 303);
  }
}

export async function devSessionRoute(c: Context): Promise<Response> {
  if (!isDevSessionEnabled()) {
    return c.json({ message: 'Development sessions are not enabled.' }, 404);
  }
  const input = await readJsonOrForm(c);
  const email = normalizeEmail(stringValue(input.email) || process.env.OOXML_DEV_AUTH_EMAIL || 'dev-ooxml@example.test');
  const returnTo = safeReturnTo(stringValue(input.returnTo));
  const user = await mutateAuthState(async (state) => findOrCreateUserByEmail(state, email, new Date()));
  await issueSession(c, user);
  if (wantsHtml(c)) return c.redirect(returnTo, 303);
  return c.json({ message: 'Development session started.', user: { id: user.id, email: user.email } });
}

export async function logoutRoute(c: Context): Promise<Response> {
  const sessionCookie = getCookie(c, SESSION_COOKIE_NAME);
  if (sessionCookie) {
    const tokenHash = hashToken(sessionCookie);
    await mutateAuthState(async (state) => {
      state.sessions = state.sessions.filter((session) => session.tokenHash !== tokenHash);
    });
  }
  deleteCookie(c, SESSION_COOKIE_NAME, { path: '/' });
  deleteCookie(c, CSRF_COOKIE_NAME, { path: '/' });
  return c.json({ message: 'Signed out.' });
}

function isPublicPath(pathname: string): boolean {
  if (pathname === '/signin' || pathname === '/health' || pathname === '/favicon.ico' || pathname === '/robots.txt') {
    return true;
  }
  return (
    pathname === '/api/auth/magic-link/request' ||
    pathname === '/api/auth/magic-link/verify' ||
    pathname === '/api/auth/dev-session' ||
    /^\/api\/auth\/oauth\/[^/]+\/(?:start|callback)$/.test(pathname)
  );
}

function isStateChangingMethod(method: string): boolean {
  return method === 'POST' || method === 'PUT' || method === 'PATCH' || method === 'DELETE';
}

async function validateAuthContext(c: AppContext): Promise<AuthContext | null> {
  const sessionToken = getCookie(c, SESSION_COOKIE_NAME);
  if (!sessionToken) return null;
  const tokenHash = hashToken(sessionToken);
  const now = new Date();
  return mutateAuthState(async (state) => {
    pruneExpired(state, now);
    const session = state.sessions.find((candidate) => candidate.tokenHash === tokenHash);
    if (!session) return null;
    const user = state.users.find((candidate) => candidate.id === session.userId);
    if (!user) return null;
    session.lastSeenAt = now.toISOString();
    session.expiresAt = new Date(now.getTime() + sessionTtlMs).toISOString();
    return { user, session };
  });
}

async function issueSession(c: Context, user: AuthUser): Promise<void> {
  const now = new Date();
  const sessionToken = randomToken();
  const csrfToken = randomUUID();
  await mutateAuthState(async (state) => {
    pruneExpired(state, now);
    state.sessions.push({
      id: randomUUID(),
      tokenHash: hashToken(sessionToken),
      userId: user.id,
      csrfToken,
      createdAt: now.toISOString(),
      expiresAt: new Date(now.getTime() + sessionTtlMs).toISOString(),
      lastSeenAt: now.toISOString(),
    });
  });

  setCookie(c, SESSION_COOKIE_NAME, sessionToken, {
    httpOnly: true,
    maxAge: Math.floor(sessionTtlMs / 1000),
    path: '/',
    sameSite: 'Lax',
    secure: isSecureCookie(),
  });
  setCookie(c, CSRF_COOKIE_NAME, csrfToken, {
    httpOnly: false,
    maxAge: csrfCookieMaxAgeSeconds,
    path: '/',
    sameSite: 'Lax',
    secure: isSecureCookie(),
  });
}

function refreshCsrfCookie(c: Context, csrfToken: string): void {
  setCookie(c, CSRF_COOKIE_NAME, csrfToken, {
    httpOnly: false,
    maxAge: csrfCookieMaxAgeSeconds,
    path: '/',
    sameSite: 'Lax',
    secure: isSecureCookie(),
  });
}

function hasValidCsrf(c: Context, auth: AuthContext): boolean {
  if (!hasAllowedOrigin(c)) return false;
  const csrfCookie = getCookie(c, CSRF_COOKIE_NAME);
  const csrfHeader = c.req.header(CSRF_HEADER_NAME);
  if (!csrfHeader || csrfHeader !== auth.session.csrfToken) return false;
  return !csrfCookie || csrfCookie === csrfHeader;
}

function hasAllowedOrigin(c: Context): boolean {
  const allowed = allowedOrigins(c);
  const origin = c.req.header('Origin');
  if (origin) return allowed.has(origin);
  const referer = c.req.header('Referer');
  if (!referer) return true;
  try {
    return allowed.has(new URL(referer).origin);
  } catch {
    return false;
  }
}

function allowedOrigins(c: Context): Set<string> {
  const origins = new Set<string>([new URL(c.req.url).origin]);
  const configured = process.env.APP_BASE_URL?.trim();
  if (configured) {
    try {
      origins.add(new URL(configured).origin);
    } catch {
      // Ignore invalid deployment config here; URL construction fails elsewhere too.
    }
  }
  if (isTruthy(process.env.OOXML_TRUST_PROXY_HEADERS)) {
    const forwardedHost = firstHeaderValue(c.req.header('x-forwarded-host')) || firstHeaderValue(c.req.header('host'));
    const forwardedProto = firstHeaderValue(c.req.header('x-forwarded-proto')) || new URL(c.req.url).protocol.replace(/:$/, '');
    if (forwardedHost && forwardedProto) {
      origins.add(`${forwardedProto}://${forwardedHost}`);
    }
  }
  return origins;
}

function firstHeaderValue(value: string | undefined): string {
  return value?.split(',')[0]?.trim() || '';
}

function hasAllowedVerificationOrigin(c: Context): boolean {
  return hasAllowedOrigin(c);
}

function unauthenticatedResponse(c: Context): Response {
  const pathname = new URL(c.req.url).pathname;
  if (pathname.startsWith('/api/') || pathname.startsWith('/flue/')) {
    return c.json({ error: 'Authentication required.' }, 401);
  }
  const params = new URLSearchParams({
    returnTo: withAppBasePath(`${pathname}${new URL(c.req.url).search}`),
  });
  return c.redirect(`${withAppBasePath('/signin')}?${params.toString()}`, 303);
}

async function checkMagicLinkRateLimit(
  email: string,
  ip: string | undefined,
): Promise<{ allowed: true } | { allowed: false; retryAfterSeconds: number }> {
  const now = new Date();
  const emailLimit = Number(process.env.OOXML_MAGIC_LINK_RATE_LIMIT_PER_HOUR || 6);
  const ipLimit = Number(process.env.OOXML_MAGIC_LINK_IP_RATE_LIMIT_PER_HOUR || 30);
  const globalLimit = Number(process.env.OOXML_MAGIC_LINK_GLOBAL_RATE_LIMIT_PER_HOUR || 300);
  const scopes = [
    { key: `email:${email}`, limit: emailLimit },
    ...(ip ? [{ key: `ip:${hashToken(ip).slice(0, 32)}`, limit: ipLimit }] : []),
    { key: 'global', limit: globalLimit },
  ];
  return mutateAuthState(async (state) => {
    const bucketStart = new Date(Math.floor(now.getTime() / magicLinkRateLimitWindowMs) * magicLinkRateLimitWindowMs);
    state.rateLimits = state.rateLimits.filter((record) => now.getTime() - Date.parse(record.bucketStart) < 24 * 60 * 60 * 1000);
    for (const scope of scopes) {
      let record = state.rateLimits.find((candidate) => candidate.key === scope.key && candidate.bucketStart === bucketStart.toISOString());
      if (!record) {
        record = { key: scope.key, bucketStart: bucketStart.toISOString(), count: 0 };
        state.rateLimits.push(record);
      }
      record.count += 1;
      if (record.count > scope.limit) {
        const retryAfterSeconds = Math.max(1, Math.ceil((bucketStart.getTime() + magicLinkRateLimitWindowMs - now.getTime()) / 1000));
        return { allowed: false, retryAfterSeconds };
      }
    }
    return { allowed: true };
  });
}

async function sendMagicLink(input: { email: string; magicLinkUrl: string; expiresAt: Date }): Promise<void> {
  const transport = (process.env.EMAIL_TRANSPORT || 'dev').toLowerCase();
  const from = process.env.EMAIL_FROM || 'no-reply@ooxml-workbench.local';
  if (transport === 'dev') {
    await appendDevMagicLink(input, from);
    return;
  }
  if (transport === 'resend') {
    await sendJsonEmail('https://api.resend.com/emails', process.env.RESEND_API_KEY, {
      from,
      to: input.email,
      subject: 'Sign in to OOXML Workbench',
      text: magicLinkText(input),
      html: magicLinkHtml(input),
    });
    return;
  }
  if (transport === 'postmark') {
    const token = requiredEnv('POSTMARK_SERVER_TOKEN', 'EMAIL_TRANSPORT=postmark');
    const body: Record<string, unknown> = {
      From: from,
      To: input.email,
      Subject: 'Sign in to OOXML Workbench',
      TextBody: magicLinkText(input),
      HtmlBody: magicLinkHtml(input),
      TrackLinks: 'None',
      TrackOpens: false,
    };
    if (process.env.POSTMARK_MESSAGE_STREAM) body.MessageStream = process.env.POSTMARK_MESSAGE_STREAM;
    const response = await fetch('https://api.postmarkapp.com/email', {
      method: 'POST',
      headers: {
        accept: 'application/json',
        'content-type': 'application/json',
        'x-postmark-server-token': token,
      },
      body: JSON.stringify(body),
    });
    if (!response.ok) throw new Error(`Postmark email send failed with status ${response.status}.`);
    return;
  }
  if (transport === 'mailgun') {
    const apiKey = requiredEnv('MAILGUN_API_KEY', 'EMAIL_TRANSPORT=mailgun');
    const domain = requiredEnv('MAILGUN_DOMAIN', 'EMAIL_TRANSPORT=mailgun');
    const baseUrl = (process.env.MAILGUN_BASE_URL || 'https://api.mailgun.net').replace(/\/+$/, '');
    const body = new URLSearchParams({
      from,
      to: input.email,
      subject: 'Sign in to OOXML Workbench',
      text: magicLinkText(input),
      html: magicLinkHtml(input),
      'o:tracking': 'no',
      'o:tracking-clicks': 'no',
      'o:tracking-opens': 'no',
    });
    const response = await fetch(`${baseUrl}/v3/${encodeURIComponent(domain)}/messages`, {
      method: 'POST',
      headers: {
        authorization: `Basic ${Buffer.from(`api:${apiKey}`).toString('base64')}`,
        'content-type': 'application/x-www-form-urlencoded',
      },
      body,
    });
    if (!response.ok) throw new Error(`Mailgun email send failed with status ${response.status}.`);
    return;
  }
  throw new Error(`Unsupported EMAIL_TRANSPORT: ${transport}`);
}

async function appendDevMagicLink(input: { email: string; magicLinkUrl: string; expiresAt: Date }, from: string): Promise<void> {
  const logPath = process.env.OOXML_MAGIC_LINK_LOG || join(dataRoot(), 'auth', 'magic-links.jsonl');
  await mkdir(dirname(logPath), { recursive: true });
  await appendFile(
    logPath,
    `${JSON.stringify({
      kind: 'magic-link',
      to: input.email,
      from,
      subject: 'Sign in to OOXML Workbench',
      magicLinkUrl: input.magicLinkUrl,
      expiresAt: input.expiresAt.toISOString(),
      sentAt: new Date().toISOString(),
    })}\n`,
    { encoding: 'utf8', mode: 0o600 },
  );
}

async function sendJsonEmail(endpoint: string, apiKey: string | undefined, body: unknown): Promise<void> {
  if (!apiKey?.trim()) throw new Error('RESEND_API_KEY is required when EMAIL_TRANSPORT=resend.');
  const response = await fetch(endpoint, {
    method: 'POST',
    headers: {
      authorization: `Bearer ${apiKey}`,
      'content-type': 'application/json',
    },
    body: JSON.stringify(body),
  });
  if (!response.ok) throw new Error(`Resend email send failed with status ${response.status}.`);
}

function magicLinkText(input: { magicLinkUrl: string; expiresAt: Date }): string {
  return `Sign in to OOXML Workbench:\n\n${input.magicLinkUrl}\n\nThis link expires at ${input.expiresAt.toISOString()}.`;
}

function magicLinkHtml(input: { magicLinkUrl: string; expiresAt: Date }): string {
  return `<p>Sign in to OOXML Workbench:</p><p><a href="${escapeHtml(input.magicLinkUrl)}">Sign in</a></p><p>This link expires at ${escapeHtml(input.expiresAt.toISOString())}.</p>`;
}

function oauthProviderConfig(provider: OAuthProvider): {
  authorizationEndpoint: string;
  clientId: string;
  clientSecret: string;
  scopes: string[];
  tokenEndpoint: string;
  userInfoEndpoint: string;
} {
  if (provider === 'microsoft') {
    const tenant = process.env.MICROSOFT_OAUTH_TENANT || process.env.AZURE_AD_TENANT_ID || 'common';
    return {
      authorizationEndpoint: `https://login.microsoftonline.com/${encodeURIComponent(tenant)}/oauth2/v2.0/authorize`,
      clientId: process.env.MICROSOFT_OAUTH_CLIENT_ID || process.env.AZURE_AD_CLIENT_ID || '',
      clientSecret: process.env.MICROSOFT_OAUTH_CLIENT_SECRET || process.env.AZURE_AD_CLIENT_SECRET || '',
      scopes: ['openid', 'email'],
      tokenEndpoint: `https://login.microsoftonline.com/${encodeURIComponent(tenant)}/oauth2/v2.0/token`,
      userInfoEndpoint: 'https://graph.microsoft.com/oidc/userinfo',
    };
  }
  return {
    authorizationEndpoint: 'https://accounts.google.com/o/oauth2/v2/auth',
    clientId: process.env.GOOGLE_OAUTH_CLIENT_ID || process.env.GOOGLE_CLIENT_ID || '',
    clientSecret: process.env.GOOGLE_OAUTH_CLIENT_SECRET || process.env.GOOGLE_CLIENT_SECRET || '',
    scopes: ['openid', 'email'],
    tokenEndpoint: 'https://oauth2.googleapis.com/token',
    userInfoEndpoint: 'https://openidconnect.googleapis.com/v1/userinfo',
  };
}

function isOAuthConfigured(provider: OAuthProvider): boolean {
  const config = oauthProviderConfig(provider);
  return Boolean(config.clientId && config.clientSecret);
}

async function exchangeOAuthCode(
  c: Context,
  provider: OAuthProvider,
  code: string,
  codeVerifier: string,
): Promise<{ accessToken: string; idTokenClaims: Record<string, unknown> | null }> {
  const config = oauthProviderConfig(provider);
  const body = new URLSearchParams({
    client_id: config.clientId,
    client_secret: config.clientSecret,
    code,
    code_verifier: codeVerifier,
    grant_type: 'authorization_code',
    redirect_uri: oauthRedirectUri(c, provider),
    scope: config.scopes.join(' '),
  });
  const response = await fetch(config.tokenEndpoint, {
    body,
    headers: {
      accept: 'application/json',
      'content-type': 'application/x-www-form-urlencoded',
    },
    method: 'POST',
  });
  if (!response.ok) throw new Error(`${provider} OAuth token exchange failed with status ${response.status}.`);
  const payload = (await response.json()) as { access_token?: unknown; id_token?: unknown };
  if (typeof payload.access_token !== 'string' || !payload.access_token) {
    throw new Error(`${provider} OAuth token response did not include an access token.`);
  }
  return {
    accessToken: payload.access_token,
    idTokenClaims: typeof payload.id_token === 'string' ? decodeJwtPayload(payload.id_token) : null,
  };
}

async function fetchOAuthUserInfo(provider: OAuthProvider, accessToken: string): Promise<Record<string, unknown>> {
  const response = await fetch(oauthProviderConfig(provider).userInfoEndpoint, {
    headers: {
      accept: 'application/json',
      authorization: `Bearer ${accessToken}`,
    },
    method: 'GET',
  });
  if (!response.ok) throw new Error(`${provider} OAuth userinfo request failed with status ${response.status}.`);
  return (await response.json()) as Record<string, unknown>;
}

async function oauthIdentityClaims(
  provider: OAuthProvider,
  token: { accessToken: string; idTokenClaims: Record<string, unknown> | null },
): Promise<Record<string, unknown>> {
  if (token.idTokenClaims) {
    const email = extractVerifiedOAuthEmail(provider, token.idTokenClaims);
    const subject = extractOAuthSubject(provider, token.idTokenClaims);
    if (email && subject) return token.idTokenClaims;
  }
  return fetchOAuthUserInfo(provider, token.accessToken);
}

function extractVerifiedOAuthEmail(provider: OAuthProvider, claims: Record<string, unknown>): string | null {
  if (provider === 'google' && !isTruthy(claims.email_verified)) return null;
  if (provider === 'microsoft' && microsoftRequiresEdov() && !isTruthy(claims.xms_edov)) return null;
  return normalizeEmail(typeof claims.email === 'string' ? claims.email : '');
}

function extractOAuthSubject(provider: OAuthProvider, claims: Record<string, unknown>): string | null {
  if (typeof claims.sub === 'string' && claims.sub.trim()) return claims.sub.trim();
  if (provider === 'microsoft' && typeof claims.oid === 'string') {
    const tenantId = typeof claims.tid === 'string' ? claims.tid.trim() : '';
    return tenantId ? `${tenantId}:${claims.oid.trim()}` : claims.oid.trim();
  }
  return null;
}

function oauthRedirectUri(c: Context, provider: OAuthProvider): string {
  const explicit =
    provider === 'microsoft'
      ? process.env.MICROSOFT_OAUTH_REDIRECT_URI || process.env.AZURE_AD_REDIRECT_URI
      : process.env.GOOGLE_OAUTH_REDIRECT_URI || process.env.GOOGLE_REDIRECT_URI;
  if (explicit?.trim()) return explicit.trim();
  return appAbsoluteUrl(c, `/api/auth/oauth/${provider}/callback`);
}

function oauthStateCookieName(provider: OAuthProvider): string {
  return `ooxml_oauth_${provider}`;
}

function encodeOAuthStateCookie(value: OAuthStateCookie): string {
  return Buffer.from(JSON.stringify(value), 'utf8').toString('base64url');
}

function decodeOAuthStateCookie(value: string): OAuthStateCookie | null {
  try {
    const parsed = JSON.parse(Buffer.from(value, 'base64url').toString('utf8')) as Partial<OAuthStateCookie>;
    if (!isOAuthProvider(parsed.provider) || typeof parsed.codeVerifier !== 'string' || typeof parsed.returnTo !== 'string' || typeof parsed.state !== 'string') {
      return null;
    }
    return {
      codeVerifier: parsed.codeVerifier,
      provider: parsed.provider,
      returnTo: safeReturnTo(parsed.returnTo),
      state: parsed.state,
    };
  } catch {
    return null;
  }
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

function pkceChallenge(codeVerifier: string): string {
  return createHash('sha256').update(codeVerifier).digest('base64url');
}

async function readJsonOrForm(c: Context): Promise<Record<string, unknown>> {
  const contentType = c.req.header('content-type') || '';
  if (contentType.includes('application/json')) {
    const parsed = (await c.req.json().catch(() => ({}))) as unknown;
    return parsed && typeof parsed === 'object' && !Array.isArray(parsed) ? (parsed as Record<string, unknown>) : {};
  }
  if (contentType.includes('application/x-www-form-urlencoded') || contentType.includes('multipart/form-data')) {
    const form = await c.req.formData().catch(() => null);
    if (!form) return {};
    return Object.fromEntries([...form.entries()].map(([key, value]) => [key, typeof value === 'string' ? value : value.name]));
  }
  return {};
}

async function mutateAuthState<T>(callback: (state: AuthState) => Promise<T> | T): Promise<T> {
  const run = async () => {
    const state = await loadAuthState();
    const result = await callback(state);
    await saveAuthState(state);
    return result;
  };
  const next = stateQueue.then(run, run);
  stateQueue = next.then(
    () => undefined,
    () => undefined,
  );
  return next;
}

async function loadAuthState(): Promise<AuthState> {
  try {
    const raw = await readFile(authJsonPath(), 'utf8');
    const parsed = JSON.parse(raw) as Partial<AuthState>;
    return {
      users: Array.isArray(parsed.users) ? parsed.users : [],
      sessions: Array.isArray(parsed.sessions) ? parsed.sessions : [],
      magicLinks: Array.isArray(parsed.magicLinks) ? parsed.magicLinks : [],
      oauthIdentities: Array.isArray(parsed.oauthIdentities) ? parsed.oauthIdentities : [],
      rateLimits: Array.isArray(parsed.rateLimits) ? parsed.rateLimits : [],
    };
  } catch (error) {
    if (error instanceof Error && 'code' in error && (error as NodeJS.ErrnoException).code === 'ENOENT') {
      return { users: [], sessions: [], magicLinks: [], oauthIdentities: [], rateLimits: [] };
    }
    throw error;
  }
}

async function saveAuthState(state: AuthState): Promise<void> {
  const path = authJsonPath();
  await mkdir(dirname(path), { recursive: true });
  await writeFile(path, `${JSON.stringify(state, null, 2)}\n`, { mode: 0o600 });
}

function authJsonPath(): string {
  return join(dataRoot(), 'auth', 'auth.json');
}

function findOrCreateUserByEmail(state: AuthState, email: string, now: Date): AuthUser {
  const normalized = normalizeEmail(email);
  const existing = state.users.find((user) => user.email === normalized);
  if (existing) {
    existing.updatedAt = now.toISOString();
    return existing;
  }
  const user = {
    id: `user-${randomUUID()}`,
    email: normalized,
    createdAt: now.toISOString(),
    updatedAt: now.toISOString(),
  };
  state.users.push(user);
  return user;
}

function pruneExpired(state: AuthState, now: Date): void {
  const nowMs = now.getTime();
  state.sessions = state.sessions.filter((session) => Date.parse(session.expiresAt) > nowMs);
  state.magicLinks = state.magicLinks.filter((token) => Date.parse(token.expiresAt) > nowMs);
}

function clientIp(headers: Headers): string | undefined {
  return headers.get('x-forwarded-for')?.split(',')[0]?.trim() || headers.get('cf-connecting-ip')?.trim() || headers.get('x-real-ip')?.trim() || undefined;
}

function randomToken(bytes = 32): string {
  return randomBytes(bytes).toString('base64url');
}

function hashToken(value: string): string {
  return createHash('sha256').update(value, 'utf8').digest('hex');
}

function normalizeEmail(email: string): string {
  return email.trim().toLowerCase();
}

function isValidEmail(email: string): boolean {
  return /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email);
}

export function isOAuthProvider(value: unknown): value is OAuthProvider {
  return value === 'google' || value === 'microsoft';
}

function safeReturnTo(value: string | null | undefined): string {
  const trimmed = value?.trim() || '/';
  if (!trimmed.startsWith('/') || trimmed.startsWith('//') || trimmed.includes('://') || trimmed.includes('\\')) return '/';
  return trimmed;
}

function stringValue(value: unknown): string {
  return typeof value === 'string' ? value : '';
}

function wantsHtml(c: Context): boolean {
  const contentType = c.req.header('content-type') || '';
  const accept = c.req.header('accept') || '';
  return contentType.includes('application/x-www-form-urlencoded') || contentType.includes('multipart/form-data') || accept.includes('text/html');
}

function isSecureCookie(): boolean {
  return process.env.NODE_ENV === 'production';
}

function isDevSessionEnabled(): boolean {
  return (
    process.env.NODE_ENV !== 'production' &&
    Boolean(process.env.OOXML_DEV_AUTH_EMAIL || process.env.OOXML_AUTH_DEV_BYPASS === '1' || process.env.OOXML_AUTH_DEV_SESSIONS === '1')
  );
}

function isTruthy(value: unknown): boolean {
  return value === true || value === 'true';
}

function microsoftRequiresEdov(): boolean {
  const value = process.env.OOXML_MICROSOFT_REQUIRE_EDOV;
  return value === undefined ? true : isTruthy(value);
}

function requiredEnv(name: string, context: string): string {
  const value = process.env[name]?.trim();
  if (!value) throw new Error(`${name} is required when ${context}.`);
  return value;
}

function escapeHtml(value: string): string {
  return value.replace(/&/g, '&amp;').replace(/"/g, '&quot;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}
