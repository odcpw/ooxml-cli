import { appPathPrefix } from './shared/app-url.ts';
import { themeCss } from './shared/theme.ts';

const LANGUAGES: Array<[string, string]> = [
  ['de', 'German'],
  ['en', 'English'],
  ['fr', 'French'],
  ['es', 'Spanish'],
  ['it', 'Italian'],
  ['pt-BR', 'Portuguese (Brazil)'],
  ['pt-PT', 'Portuguese (Portugal)'],
  ['nl', 'Dutch'],
  ['pl', 'Polish'],
  ['sv', 'Swedish'],
  ['da', 'Danish'],
  ['nb', 'Norwegian'],
  ['fi', 'Finnish'],
  ['cs', 'Czech'],
  ['tr', 'Turkish'],
  ['ja', 'Japanese'],
  ['zh-CN', 'Chinese (Simplified)'],
  ['ko', 'Korean'],
  ['ar', 'Arabic'],
];

/**
 * Drag-and-drop presentation translation. One PPTX/PPTM in, one translated
 * copy out, through `pptx translate export` -> model -> `pptx translate apply`
 * with strict validation before anything is offered for download.
 */
export function translateHtml(): string {
  const basePath = appPathPrefix();
  const targetOptions = LANGUAGES.map(([tag, name]) => `<option value="${tag}">${name} (${tag})</option>`).join('');
  const sourceOptions = LANGUAGES.map(([tag, name]) => `<option value="${tag}">${name} (${tag})</option>`).join('');
  return `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Translate a presentation</title>
  <style>
${themeCss()}
    body { margin: 0; background: var(--color-surface); color: var(--color-text); font-family: var(--font-sans); }
    .wrap { max-width: 720px; margin: 0 auto; padding: var(--space-6) var(--space-4); }
    header { display: flex; align-items: baseline; justify-content: space-between; gap: var(--space-3); margin-bottom: var(--space-4); }
    header h1 { font-size: 1.25rem; margin: 0; }
    header a { color: var(--color-muted); font-size: 0.9rem; }
    .card { background: var(--color-surface-elev); border: 1px solid var(--color-border); border-radius: var(--radius-lg, 12px); padding: var(--space-4); }
    .drop { border: 2px dashed var(--color-border); border-radius: var(--radius-lg, 12px); padding: var(--space-6) var(--space-4); text-align: center; cursor: pointer; transition: border-color 120ms, background 120ms; }
    .drop.active { border-color: var(--color-accent); background: color-mix(in srgb, var(--color-accent) 8%, transparent); }
    .drop strong { display: block; margin-bottom: var(--space-1); }
    .drop .file { margin-top: var(--space-2); font-family: var(--font-mono); font-size: 0.85rem; color: var(--color-muted); }
    .grid { display: grid; grid-template-columns: 1fr 1fr; gap: var(--space-3); margin-top: var(--space-4); }
    label { display: block; font-size: 0.85rem; color: var(--color-muted); margin-bottom: var(--space-1); }
    select, input[type="text"] { width: 100%; box-sizing: border-box; padding: 0.5rem 0.6rem; border: 1px solid var(--color-border); border-radius: var(--radius-md, 8px); background: var(--color-bg); color: var(--color-text); font: inherit; }
    .row { display: flex; align-items: center; gap: var(--space-3); margin-top: var(--space-4); flex-wrap: wrap; }
    .check { display: flex; align-items: center; gap: var(--space-2); font-size: 0.9rem; color: var(--color-muted); }
    button.primary { background: var(--color-accent); color: #fff; border: 0; border-radius: var(--radius-md, 8px); padding: 0.6rem 1.1rem; font: inherit; font-weight: 600; cursor: pointer; }
    button.primary[disabled] { opacity: 0.6; cursor: default; }
    .status { margin-top: var(--space-4); font-size: 0.95rem; min-height: 1.4em; }
    .status.error { color: var(--color-danger); }
    .status.ok { color: var(--color-success); }
    .result { margin-top: var(--space-3); display: none; }
    .result.show { display: block; }
    .result a.download { display: inline-block; background: var(--color-success); color: #fff; border-radius: var(--radius-md, 8px); padding: 0.6rem 1.1rem; font-weight: 600; text-decoration: none; }
    .meta { margin-top: var(--space-2); font-size: 0.85rem; color: var(--color-muted); font-family: var(--font-mono); }
    .note { margin-top: var(--space-4); font-size: 0.85rem; color: var(--color-muted); }
    @media (max-width: 560px) { .grid { grid-template-columns: 1fr; } }
  </style>
</head>
<body>
  <div class="wrap">
    <header>
      <h1>Translate a presentation</h1>
      <a href="${basePath}/">Open the workbench</a>
    </header>
    <form id="translateForm" class="card">
      <div id="drop" class="drop" tabindex="0" role="button" aria-label="Drop a PPTX file or click to choose one">
        <strong>Drop a .pptx here</strong>
        <span>or click to choose a file</span>
        <div id="fileName" class="file">No file selected.</div>
        <input id="fileInput" type="file" accept=".pptx,.pptm" hidden />
      </div>
      <div class="grid">
        <div>
          <label for="targetLang">Translate into</label>
          <select id="targetLang" required>${targetOptions}</select>
        </div>
        <div>
          <label for="sourceLang">Source language (optional)</label>
          <select id="sourceLang"><option value="">Detect automatically</option>${sourceOptions}</select>
        </div>
      </div>
      <div class="row">
        <label class="check"><input id="includeNotes" type="checkbox" checked /> Also translate speaker notes</label>
        <button id="submitBtn" class="primary" type="submit" disabled>Translate</button>
      </div>
      <div id="status" class="status" aria-live="polite"></div>
      <div id="result" class="result">
        <a id="downloadLink" class="download" href="#" download>Download translated presentation</a>
        <div id="meta" class="meta"></div>
      </div>
    </form>
    <p class="note">Slide text and notes are translated; layout, images and charts are kept. The translated copy is strictly validated before download and is kept in your workbench library. Do not upload unnecessary personal data.</p>
  </div>
  <script>
    const APP_BASE_PATH = ${JSON.stringify(basePath)};
    const form = document.getElementById('translateForm');
    const drop = document.getElementById('drop');
    const fileInput = document.getElementById('fileInput');
    const fileName = document.getElementById('fileName');
    const targetLang = document.getElementById('targetLang');
    const sourceLang = document.getElementById('sourceLang');
    const includeNotes = document.getElementById('includeNotes');
    const submitBtn = document.getElementById('submitBtn');
    const status = document.getElementById('status');
    const result = document.getElementById('result');
    const downloadLink = document.getElementById('downloadLink');
    const meta = document.getElementById('meta');
    let file = null;
    let csrfToken = '';

    function setFile(candidate) {
      if (!candidate) return;
      const name = String(candidate.name || '');
      if (!/\\.(pptx|pptm)$/i.test(name)) {
        setStatus('Only .pptx and .pptm presentations can be translated here.', 'error');
        return;
      }
      file = candidate;
      fileName.textContent = name + ' (' + Math.max(1, Math.round(candidate.size / 1024)) + ' KB)';
      submitBtn.disabled = false;
      setStatus('');
      result.classList.remove('show');
    }

    function setStatus(text, kind) {
      status.textContent = text;
      status.className = 'status' + (kind ? ' ' + kind : '');
    }

    drop.addEventListener('click', () => fileInput.click());
    drop.addEventListener('keydown', (event) => { if (event.key === 'Enter' || event.key === ' ') { event.preventDefault(); fileInput.click(); } });
    fileInput.addEventListener('change', () => setFile(fileInput.files && fileInput.files[0]));
    for (const name of ['dragenter', 'dragover']) {
      drop.addEventListener(name, (event) => { event.preventDefault(); drop.classList.add('active'); });
    }
    for (const name of ['dragleave', 'dragend', 'drop']) {
      drop.addEventListener(name, (event) => { event.preventDefault(); drop.classList.remove('active'); });
    }
    drop.addEventListener('drop', (event) => {
      const dropped = event.dataTransfer && event.dataTransfer.files ? event.dataTransfer.files[0] : null;
      setFile(dropped);
    });
    window.addEventListener('dragover', (event) => event.preventDefault());
    window.addEventListener('drop', (event) => event.preventDefault());

    form.addEventListener('submit', async (event) => {
      event.preventDefault();
      if (!file) return;
      submitBtn.disabled = true;
      result.classList.remove('show');
      setStatus('Uploading and translating. Large decks can take a minute or two.');
      try {
        const body = new FormData();
        body.set('file', file, file.name);
        body.set('targetLang', targetLang.value);
        body.set('sourceLang', sourceLang.value);
        body.set('includeNotes', includeNotes.checked ? '1' : '0');
        const response = await apiFetch('/api/translate', { method: 'POST', body });
        const data = await response.json().catch(() => ({}));
        if (!response.ok) {
          throw new Error(data && data.error ? data.error : 'Translation failed (' + response.status + ').');
        }
        downloadLink.href = appUrl(data.downloadUrl);
        const applied = data.apply && typeof data.apply.entriesApplied === 'number' ? data.apply.entriesApplied : null;
        meta.textContent = [
          'segments: ' + data.entryCount,
          applied === null ? null : 'applied: ' + applied,
          'target: ' + data.targetLang,
          'validated: ' + (data.validate && data.validate.valid === true ? 'strict' : 'no'),
        ].filter(Boolean).join('  ·  ');
        result.classList.add('show');
        setStatus('Done. The translated copy also appears in your workbench library.', 'ok');
      } catch (error) {
        setStatus(error && error.message ? error.message : String(error), 'error');
      } finally {
        submitBtn.disabled = !file;
      }
    });

    async function apiFetch(url, options) {
      const init = Object.assign({}, options || {});
      const headers = new Headers(init.headers || {});
      if (!headers.has('accept')) headers.set('accept', 'application/json');
      const csrf = cookieValue('ooxml_csrf') || csrfToken || await refreshCsrfToken();
      if (csrf) headers.set('x-ooxml-csrf', csrf);
      init.headers = headers;
      const response = await fetch(appUrl(url), init);
      if (response.status === 401) {
        window.location.href = appUrl('/signin?returnTo=' + encodeURIComponent(window.location.pathname + window.location.search));
      }
      return response;
    }

    async function refreshCsrfToken() {
      try {
        const response = await fetch(appUrl('/api/auth/me'), { headers: { accept: 'application/json' } });
        if (!response.ok) return '';
        const data = await response.json().catch(() => ({}));
        if (data && data.csrfToken) csrfToken = data.csrfToken;
        return csrfToken || cookieValue('ooxml_csrf');
      } catch {
        return '';
      }
    }

    function cookieValue(name) {
      const match = document.cookie.split('; ').find((entry) => entry.startsWith(name + '='));
      return match ? decodeURIComponent(match.slice(name.length + 1)) : '';
    }

    function appUrl(value) {
      const url = String(value || '/');
      if (url.startsWith(APP_BASE_PATH + '/') || url === APP_BASE_PATH) return url;
      return APP_BASE_PATH + (url.startsWith('/') ? url : '/' + url);
    }
  </script>
</body>
</html>`;
}
