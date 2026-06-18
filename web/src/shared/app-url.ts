import type { Context } from 'hono';

export function appPathPrefix(): string {
  const explicit = normalizePathPrefix(process.env.OOXML_WEB_BASE_PATH);
  if (explicit !== null) return explicit;

  const baseUrl = process.env.APP_BASE_URL?.trim();
  if (!baseUrl) return '';
  try {
    return normalizePathPrefix(new URL(baseUrl).pathname) ?? '';
  } catch {
    return '';
  }
}

export function withAppBasePath(path: string): string {
  const prefix = appPathPrefix();
  const normalized = normalizeAbsolutePath(path);
  if (!prefix) return normalized;
  if (normalized === prefix || normalized.startsWith(`${prefix}/`)) return normalized;
  return normalized === '/' ? prefix : `${prefix}${normalized}`;
}

export function appAbsoluteUrl(c: Context, path: string): string {
  const configured = process.env.APP_BASE_URL?.trim();
  const origin = configured ? new URL(configured).origin : new URL(c.req.url).origin;
  return new URL(withAppBasePath(path), origin).toString();
}

function normalizePathPrefix(value: string | undefined): string | null {
  const trimmed = value?.trim();
  if (!trimmed) return null;
  const pathname = trimmed.startsWith('http://') || trimmed.startsWith('https://')
    ? new URL(trimmed).pathname
    : trimmed;
  const normalized = normalizeAbsolutePath(pathname).replace(/\/+$/, '');
  return normalized === '/' ? '' : normalized;
}

function normalizeAbsolutePath(path: string): string {
  const trimmed = path.trim() || '/';
  return `/${trimmed.replace(/^\/+/, '')}`;
}
