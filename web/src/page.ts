import { appPathPrefix } from './shared/app-url.ts';
import {
  previewAvailableOnlyCopy,
  previewInspectCopy,
  previewRenderPromptCopy,
  previewSupportedLabel,
  previewWiredOnlyCopy,
  uploadAcceptAttribute,
} from './shared/file-support.ts';
import { themeCss } from './shared/theme.ts';

export function workbenchHtml(): string {
  const basePath = appPathPrefix();
  return `<!doctype html>
<html lang="en" class="dark">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>OOXML Agent Workbench</title>
    <style>
${themeCss()}
      :root { --thumb-width: 280px; }
      html, body { margin: 0; min-height: 100%; overflow: hidden; }
      body {
        min-height: 100vh;
        -webkit-font-smoothing: antialiased;
      }
      button, input, textarea { font: inherit; }
      button {
        border: var(--border-width) solid transparent;
        border-radius: var(--radius-md);
        padding: 0 var(--space-3);
        min-height: var(--control-h-md);
        background: var(--color-accent);
        color: var(--color-bg);
        font-weight: var(--font-weight-medium);
        cursor: pointer;
        transition: background-color var(--duration-fast) var(--ease-standard),
          border-color var(--duration-fast) var(--ease-standard), opacity var(--duration-fast) var(--ease-standard);
      }
      button:hover:not(:disabled) { opacity: .9; }
      button.secondary {
        background: var(--color-surface);
        border-color: var(--color-border);
        color: var(--color-text);
      }
      button.secondary:hover:not(:disabled) {
        border-color: var(--color-accent);
        background: var(--color-surface-elev);
        opacity: 1;
      }
      button.ghost {
        background: transparent;
        border-color: transparent;
        color: var(--color-muted);
      }
      button.ghost:hover:not(:disabled) {
        background: var(--color-surface);
        color: var(--color-text);
        opacity: 1;
      }
      button.danger {
        background: var(--color-surface);
        border-color: var(--color-border);
        color: var(--color-danger);
      }
      button.danger:hover:not(:disabled) {
        border-color: var(--color-danger);
        background: color-mix(in srgb, var(--color-danger) 12%, var(--color-surface));
        opacity: 1;
      }
      button:disabled { opacity: .6; cursor: not-allowed; }
      input[type="file"], input[type="text"], textarea {
        width: 100%;
        border: var(--border-width) solid var(--color-border);
        border-radius: var(--radius-md);
        background: var(--color-surface);
        color: var(--color-text);
        font-size: var(--text-sm);
        padding: var(--space-2) var(--space-3);
        box-shadow: var(--shadow-sm);
        transition: border-color var(--duration-fast) var(--ease-standard), box-shadow var(--duration-fast) var(--ease-standard);
      }
      input[type="text"], input[type="file"] { min-height: var(--control-h-md); }
      input[type="file"], input[type="text"], textarea { outline: none; }
      input[type="text"]::placeholder, textarea::placeholder { color: var(--color-muted); }
      input[type="text"]:focus, textarea:focus {
        border-color: var(--color-accent);
        box-shadow: 0 0 0 var(--ring-width) color-mix(in srgb, var(--color-accent) 35%, transparent);
      }
      input[type="range"] { width: 150px; accent-color: var(--color-accent); }
      textarea { min-height: 90px; resize: vertical; line-height: var(--leading-snug); }
      .app {
        display: grid;
        grid-template-columns: 280px minmax(360px, 520px) minmax(0, 1fr);
        height: 100vh;
        overflow: hidden;
      }
      .pane {
        min-height: 0;
        border-right: var(--border-width) solid var(--color-border);
        background: var(--color-surface);
      }
      .threads {
        display: grid;
        grid-template-rows: auto 1fr auto;
        min-height: 0;
      }
      .pane-head {
        padding: var(--space-4);
        border-bottom: var(--border-width) solid var(--color-border);
      }
      .brand {
        font-size: var(--text-lg);
        font-weight: var(--font-weight-semibold);
        letter-spacing: var(--tracking-tight);
      }
      .subtle { color: var(--color-muted); font-size: var(--text-xs); line-height: var(--leading-snug); }
      .thread-list, .doc-list, .chat-log { overflow: auto; }
      .thread-list { padding: var(--space-3); }
      .thread-row {
        width: 100%;
        display: block;
        text-align: left;
        border: var(--border-width) solid transparent;
        background: transparent;
        color: var(--color-text);
        padding: var(--space-3);
        min-height: 0;
        border-radius: var(--radius-md);
        margin-bottom: var(--space-2);
      }
      .thread-row:hover, .thread-row.current {
        background: var(--color-surface-elev);
        border-color: var(--color-border);
        opacity: 1;
      }
      .thread-title, .doc-title {
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
        font-size: var(--text-sm);
        font-weight: var(--font-weight-medium);
      }
      .thread-meta, .doc-meta {
        margin-top: var(--space-1);
        color: var(--color-muted);
        font-size: var(--text-xs);
      }
      .work {
        display: grid;
        grid-template-rows: auto auto 1fr auto;
        min-height: 0;
      }
      .section { padding: var(--space-4); border-bottom: var(--border-width) solid var(--color-border); }
      .account-strip {
        display: grid;
        grid-template-columns: minmax(0, 1fr) auto;
        gap: var(--space-2);
        align-items: center;
        margin-top: var(--space-3);
        padding-top: var(--space-3);
        border-top: var(--border-width) solid var(--color-border);
      }
      .account-strip button { padding: 0 var(--space-2); min-height: var(--control-h-sm); font-size: var(--text-xs); }
      .section-title {
        margin: 0 0 var(--space-3);
        font-size: var(--text-xs);
        color: var(--color-muted);
        text-transform: uppercase;
        letter-spacing: var(--tracking-wide);
      }
      .privacy-note {
        margin-top: var(--space-3);
        color: var(--color-muted);
        font-size: var(--text-xs);
        line-height: var(--leading-snug);
      }
      .privacy-note a {
        color: var(--color-accent);
      }
      .upload-form { display: grid; gap: var(--space-2); }
      .row { display: flex; gap: var(--space-2); align-items: center; flex-wrap: wrap; }
      .composer-row { justify-content: space-between; }
      .activity-console {
        margin-top: var(--space-2);
        border: var(--border-width) solid var(--color-border);
        border-radius: var(--radius-lg);
        background: var(--color-bg);
        overflow: hidden;
      }
      .activity-head {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: var(--space-2);
        padding: var(--space-2) var(--space-3);
        border-bottom: var(--border-width) solid var(--color-border);
      }
      .activity-title {
        color: var(--color-muted);
        font-size: var(--text-xs);
        font-weight: var(--font-weight-semibold);
        letter-spacing: var(--tracking-wide);
        text-transform: uppercase;
      }
      .activity-log {
        height: 72px;
        overflow: auto;
        padding: var(--space-2) var(--space-3);
        font-family: var(--font-mono);
        font-size: var(--text-xs);
        line-height: var(--leading-snug);
      }
      .activity-line {
        display: grid;
        grid-template-columns: 54px minmax(0, 1fr);
        gap: var(--space-2);
        min-height: 16px;
        color: var(--color-muted);
      }
      .activity-line.error { color: var(--color-danger); }
      .activity-time { color: var(--color-muted); }
      .activity-text {
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
      }
      .status-line {
        display: inline-flex;
        align-items: center;
        gap: var(--space-2);
        color: var(--color-muted);
        font-size: var(--text-xs);
        min-width: 150px;
      }
      .status-dot {
        width: var(--space-2);
        height: var(--space-2);
        border-radius: var(--radius-full);
        background: var(--color-muted);
      }
      .status-dot.running {
        background: var(--color-success);
        box-shadow: 0 0 0 4px color-mix(in srgb, var(--color-success) 14%, transparent);
      }
      .doc-list { max-height: 260px; display: grid; gap: var(--space-2); }
      .doc-card {
        display: grid;
        grid-template-columns: minmax(0, 1fr) auto;
        gap: var(--space-2);
        align-items: center;
        border: var(--border-width) solid var(--color-border);
        border-radius: var(--radius-lg);
        padding: var(--space-3);
        background: var(--color-surface-elev);
      }
      .doc-card.current {
        border-color: var(--color-accent);
        background: color-mix(in srgb, var(--color-accent) 12%, var(--color-surface));
      }
      .doc-actions { display: flex; gap: var(--space-2); align-items: center; }
      .doc-actions button { padding: 0 var(--space-2); min-height: var(--control-h-sm); font-size: var(--text-xs); }
      .chat-log {
        padding: var(--space-4);
        display: flex;
        flex-direction: column;
        gap: var(--space-3);
      }
      .message {
        border: var(--border-width) solid var(--color-border);
        border-radius: var(--radius-lg);
        background: var(--color-surface-elev);
        padding: var(--space-3);
        font-size: var(--text-sm);
        line-height: var(--leading-snug);
        white-space: normal;
      }
      .message.user {
        background: color-mix(in srgb, var(--color-success) 10%, var(--color-surface));
        border-color: color-mix(in srgb, var(--color-success) 45%, var(--color-border));
      }
      .message.assistant {
        background: var(--color-surface);
      }
      .message.trace {
        background: var(--color-surface);
        border-color: var(--color-border);
        color: var(--color-muted);
        font-size: var(--text-xs);
        padding: var(--space-2) var(--space-3);
      }
      .message.error {
        background: color-mix(in srgb, var(--color-danger) 12%, var(--color-surface));
        border-color: color-mix(in srgb, var(--color-danger) 50%, var(--color-border));
        color: var(--color-danger);
      }
      .message p { margin: 0 0 var(--space-2); }
      .message p:last-child { margin-bottom: 0; }
      .message ul { margin: var(--space-2) 0 var(--space-2) var(--space-5); padding: 0; }
      .message code {
        font-family: var(--font-mono);
        background: var(--color-surface-elev);
        border: var(--border-width) solid var(--color-border);
        border-radius: var(--radius-sm);
        padding: 1px 4px;
        font-size: var(--text-xs);
      }
      .composer {
        padding: var(--space-4);
        border-top: var(--border-width) solid var(--color-border);
        background: var(--color-surface);
      }
      .preview-pane {
        min-height: 0;
        background: var(--color-bg);
        display: grid;
        grid-template-rows: auto 1fr;
      }
      .preview-head {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: var(--space-4);
        padding: var(--space-4);
        border-bottom: var(--border-width) solid var(--color-border);
        background: var(--color-surface);
      }
      .preview-title { font-size: var(--text-base); font-weight: var(--font-weight-semibold); }
      .preview-body {
        overflow: auto;
        padding: var(--space-4);
        min-height: 0;
      }
      .thumbs {
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(min(var(--thumb-width), 100%), var(--thumb-width)));
        gap: var(--space-4);
        align-items: start;
        justify-content: start;
      }
      .thumb {
        border: var(--border-width) solid var(--color-border);
        border-radius: var(--radius-lg);
        padding: var(--space-2);
        background: var(--color-surface);
      }
      .thumb img {
        display: block;
        width: 100%;
        height: auto;
        border-radius: var(--radius-sm);
        background: var(--color-bg);
      }
      .empty {
        color: var(--color-muted);
        border: var(--border-width) dashed var(--color-border);
        border-radius: var(--radius-lg);
        background: var(--color-surface);
        padding: var(--space-4);
      }
      a#downloadLink {
        display: inline-flex;
        align-items: center;
        justify-content: center;
        min-height: var(--control-h-sm);
        padding: 0 0.625rem;
        border: var(--border-width) solid var(--color-border);
        border-radius: var(--radius-md);
        background: var(--color-surface);
        color: var(--color-text);
        font-size: var(--text-sm);
        font-weight: var(--font-weight-medium);
        text-decoration: none;
      }
      a#downloadLink:hover { border-color: var(--color-accent); background: var(--color-surface-elev); }
      a#downloadLink[hidden] { display: none; }
      @media (max-width: 1180px) {
        .app { grid-template-columns: 240px minmax(340px, 460px) minmax(0, 1fr); }
      }
      @media (max-width: 900px) {
        .app { grid-template-columns: 1fr; }
        html, body { overflow: auto; }
        .app { height: auto; min-height: 100vh; overflow: visible; }
        .pane, .work, .preview-pane { min-height: auto; }
        .threads { max-height: 320px; }
      }
    </style>
  </head>
  <body>
    <div class="app">
      <aside class="pane threads">
        <div class="pane-head">
          <div class="brand">OOXML Workbench</div>
          <div class="subtle">Threads, file libraries, agent traces.</div>
          <div class="account-strip">
            <div id="accountLine" class="subtle">Signed in.</div>
            <button id="logoutBtn" class="secondary" type="button">Sign out</button>
          </div>
        </div>
        <div id="threadList" class="thread-list"></div>
        <div class="section">
          <button id="newThreadBtn" class="secondary" type="button">New thread</button>
        </div>
      </aside>

      <section class="pane work">
        <div class="section">
          <h2 class="section-title">Upload</h2>
          <form id="uploadForm" class="upload-form">
            <input id="titleInput" type="text" placeholder="Thread title" aria-label="Thread title" />
            <input id="fileInput" type="file" accept="${uploadAcceptAttribute}" multiple required aria-label="Office files to upload" />
            <button id="uploadBtn" type="submit">Upload file(s)</button>
          </form>
          <div class="privacy-note">
            Do not upload unnecessary personal data or sensitive PII.
            <a href="${basePath}/privacy">Privacy</a>
          </div>
        </div>
        <div class="section">
          <h2 class="section-title">Library</h2>
          <div id="threadInfo" class="subtle">No thread selected.</div>
          <div id="documentList" class="doc-list"></div>
          <div class="row" style="margin-top:var(--space-2)">
            <button id="refreshBtn" class="secondary" disabled>Refresh</button>
            <button id="renderBtn" class="secondary" disabled>Render preview</button>
            <a id="downloadLink" class="secondary" href="#" hidden>Download current</a>
          </div>
        </div>
        <div id="chat" class="chat-log"></div>
        <form id="chatForm" class="composer">
	          <textarea id="promptInput" placeholder="Ask the agent to translate slides, inspect, validate, render, search, or make exact text changes..." aria-label="Message the agent" disabled></textarea>
          <div class="activity-console" aria-label="Agent activity">
            <div class="activity-head">
              <div class="activity-title">Agent activity</div>
            </div>
            <div id="activityLog" class="activity-log" role="log" aria-live="polite" aria-relevant="additions"></div>
          </div>
	          <div class="row composer-row" style="margin-top:var(--space-2)">
	            <div class="row">
	              <button id="sendBtn" type="submit" disabled>Send</button>
	              <button id="stopBtn" class="secondary" type="button" hidden>Stop</button>
	            </div>
	            <div class="status-line" aria-live="polite" aria-atomic="true">
	              <span id="statusDot" class="status-dot"></span>
	              <span id="statusText">Upload a file to begin.</span>
	            </div>
	          </div>
	        </form>
      </section>

      <main class="preview-pane">
        <div class="preview-head">
          <div>
            <div class="preview-title">Preview</div>
            <div id="previewMeta" class="subtle">${previewInspectCopy}</div>
          </div>
          <div class="row">
            <button id="zoomOutBtn" class="secondary" type="button">-</button>
            <input id="zoomRange" type="range" min="180" max="720" step="20" value="280" aria-label="Preview zoom" />
            <button id="zoomInBtn" class="secondary" type="button">+</button>
          </div>
        </div>
        <div id="preview" class="preview-body"></div>
      </main>
    </div>
    <script>
      const APP_BASE_PATH = ${JSON.stringify(basePath)};
		      const state = { threads: [], thread: null, thumbWidth: 280, busy: false, busyLabel: 'Working', stopStream: null, csrfToken: '', activityLines: [] };
      const threadList = document.getElementById('threadList');
      const newThreadBtn = document.getElementById('newThreadBtn');
      const uploadForm = document.getElementById('uploadForm');
      const titleInput = document.getElementById('titleInput');
      const fileInput = document.getElementById('fileInput');
      const uploadBtn = document.getElementById('uploadBtn');
      const accountLine = document.getElementById('accountLine');
      const logoutBtn = document.getElementById('logoutBtn');
      const chatForm = document.getElementById('chatForm');
	      const promptInput = document.getElementById('promptInput');
	      const sendBtn = document.getElementById('sendBtn');
	      const stopBtn = document.getElementById('stopBtn');
      const refreshBtn = document.getElementById('refreshBtn');
      const renderBtn = document.getElementById('renderBtn');
      const chat = document.getElementById('chat');
      const preview = document.getElementById('preview');
      const threadInfo = document.getElementById('threadInfo');
      const documentList = document.getElementById('documentList');
      const activityLog = document.getElementById('activityLog');
      const previewMeta = document.getElementById('previewMeta');
      const downloadLink = document.getElementById('downloadLink');
	      const zoomRange = document.getElementById('zoomRange');
	      const zoomOutBtn = document.getElementById('zoomOutBtn');
	      const zoomInBtn = document.getElementById('zoomInBtn');
	      const statusDot = document.getElementById('statusDot');
	      const statusText = document.getElementById('statusText');

      resetActivity('Idle');
	      loadAccount().catch(() => undefined);
	      loadThreads().catch((error) => {
	        addMessage('error', error.message || String(error));
	        updateStatus();
	      });

      logoutBtn.addEventListener('click', async () => {
        await apiFetch('/api/auth/logout', { method: 'POST' }).catch(() => undefined);
        window.location.href = appUrl('/signin');
      });

      newThreadBtn.addEventListener('click', () => {
        state.thread = null;
        chat.innerHTML = '';
        titleInput.value = '';
        uploadBtn.textContent = 'Upload file(s)';
        renderThreads();
        renderThread();
      });

      uploadForm.addEventListener('submit', async (event) => {
        event.preventDefault();
        const files = Array.from(fileInput.files || []);
        if (!files.length) return;
        const form = new FormData();
        for (const file of files) form.append('files', file);
        form.set('title', titleInput.value);
	        addMessage('trace', 'uploading ' + files.length + ' file(s)');
	        setBusy(true, 'Uploading file(s)');
	        try {
          const url = state.thread ? '/api/threads/' + encodeURIComponent(state.thread.id) + '/upload' : '/api/upload';
          const response = await apiFetch(url, { method: 'POST', body: form });
          const data = await readApiJson(response, 'Upload');
          state.thread = data;
          fileInput.value = '';
          titleInput.value = '';
	          addMessage('assistant', state.thread.documents.length + ' file(s) in this thread. Ask the agent what to do next.');
	          await loadThreads(data.id);
        } catch (error) {
          addMessage('error', error.message || String(error));
        } finally {
          setBusy(false);
        }
      });

	      refreshBtn.addEventListener('click', () => refreshThread());
	      stopBtn.addEventListener('click', () => {
	        if (typeof state.stopStream === 'function') {
	          state.stopStream();
	        } else {
	          setBusy(false);
	        }
	      });
	      renderBtn.addEventListener('click', async () => {
	        if (!state.thread) return;
	        if (!canRenderCurrent()) {
	          addMessage('trace', 'preview skipped · ' + ${JSON.stringify(previewSupportedLabel)} + ' thumbnails only');
	          return;
	        }
	        addMessage('trace', 'rendering preview');
	        setBusy(true, 'Rendering preview');
	        try {
	          const response = await apiFetch('/api/threads/' + encodeURIComponent(state.thread.id) + '/render', { method: 'POST' });
	          const data = await readApiJson(response, 'Render');
	          if (data.rendered === false) {
	            addMessage('trace', 'preview not rendered · ' + (data.reason || 'unsupported file type'));
	            return;
	          }
	          addMessage('trace', 'preview rendered · ' + ((data.thumbnails || []).length || 0) + ' thumbnail(s)');
	          await refreshThread();
        } catch (error) {
          addMessage('error', error.message || String(error));
        } finally {
          setBusy(false);
        }
      });

      chatForm.addEventListener('submit', async (event) => {
        event.preventDefault();
        if (!state.thread) return;
        const message = promptInput.value.trim();
        if (!message) return;
        const threadId = state.thread.id;
        promptInput.value = '';
        resetActivity('Request queued');
        addMessage('user', message);
	        setBusy(true, 'Agent working');
	        try {
          const response = await apiFetch('/flue/agents/ooxml-editor/' + encodeURIComponent(threadId), {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ message })
          });
          const data = await readApiJson(response, 'Agent request', agentErrorMessage);
          if (data.submissionId) addMessage('trace', 'submission accepted · ' + String(data.submissionId).slice(0, 8));
          await streamAgentEvents(data);
        } catch (error) {
          addMessage('error', error.message || String(error));
        } finally {
          // Reconcile with server state after ANY outcome: the submission may
          // have published a new version even if the live stream dropped, and
          // the user may have cleared/switched the thread mid-turn.
          if (state.thread && state.thread.id === threadId) {
            await refreshThread().catch(() => undefined);
          }
          await loadThreads(threadId, false).catch(() => undefined);
          setBusy(false);
        }
      });

      zoomRange.addEventListener('input', () => setZoom(Number(zoomRange.value)));
      zoomOutBtn.addEventListener('click', () => setZoom(Math.max(180, state.thumbWidth - 40)));
      zoomInBtn.addEventListener('click', () => setZoom(Math.min(720, state.thumbWidth + 40)));

      async function loadAccount() {
        const response = await apiFetch('/api/auth/me');
        const data = await readApiJson(response, 'Current user');
        if (data.csrfToken) state.csrfToken = data.csrfToken;
        if (response.ok && data.user?.email) {
          accountLine.textContent = data.user.email;
        }
      }

      async function loadThreads(selectId, loadSelected = true) {
        const response = await apiFetch('/api/threads');
        const data = await readApiJson(response, 'Thread list');
        state.threads = data.threads || [];
        const targetId = selectId || state.thread?.id || state.threads[0]?.id;
        if (targetId && loadSelected) {
          const selected = state.threads.find((thread) => thread.id === targetId);
          if (selected) state.thread = selected;
        }
        renderThreads();
        renderThread();
      }

	      async function openThread(threadId, options = {}) {
	        const response = await apiFetch('/api/threads/' + encodeURIComponent(threadId));
	        const data = await readApiJson(response, 'Thread');
	        state.thread = data;
	        if (options.clearChat !== false) {
	          chat.innerHTML = '';
	          addMessage('trace', 'opened thread · ' + data.title);
	        }
	        renderThreads();
	        renderThread();
	      }

	      async function refreshThread() {
	        if (!state.thread) return;
	        await openThread(state.thread.id, { clearChat: false });
	      }

      function renderThreads() {
        threadList.innerHTML = '';
        if (!state.threads.length) {
          threadList.innerHTML = '<div class="empty">No threads yet.</div>';
          return;
        }
        for (const thread of state.threads) {
          const button = document.createElement('button');
          button.type = 'button';
          button.className = 'thread-row' + (state.thread?.id === thread.id ? ' current' : '');
          button.innerHTML = '<div class="thread-title"></div><div class="thread-meta"></div>';
          button.querySelector('.thread-title').textContent = thread.title || thread.id;
          button.querySelector('.thread-meta').textContent = (thread.documents?.length || 1) + ' file(s) · ' + (thread.currentVersionId || '');
          button.addEventListener('click', () => openThread(thread.id));
          threadList.append(button);
        }
      }

      function renderThread() {
        const thread = state.thread;
        const enabled = Boolean(thread);
	        promptInput.disabled = !enabled;
	        sendBtn.disabled = !enabled;
	        refreshBtn.disabled = !enabled;
	        renderBtn.disabled = !enabled || !canRenderThread(thread);
	        documentList.innerHTML = '';
	        downloadLink.hidden = !enabled;
	        renderBtn.title = enabled && !canRenderThread(thread) ? ${JSON.stringify(previewAvailableOnlyCopy)} : '';

        if (!thread) {
          threadInfo.textContent = 'No thread selected.';
          preview.innerHTML = '<div class="empty">Upload files or open a thread to begin.</div>';
          previewMeta.textContent = ${JSON.stringify(previewInspectCopy)};
	          uploadBtn.textContent = 'Upload file(s)';
	          updateStatus();
	          return;
	        }

        const documents = thread.documents || [];
        const currentDocument = documents.find((document) => document.id === thread.currentDocumentId);
        threadInfo.textContent = thread.title + ' · ' + (thread.currentDocumentName || currentDocument?.originalName || thread.currentFile || '');
        uploadBtn.textContent = 'Add file(s)';
        downloadLink.href = appUrl(thread.downloadUrl);
	        renderDocumentList(documents, thread.currentDocumentId);
	        renderPreview(thread, currentDocument);
	        updateStatus();
	      }

      function renderDocumentList(documents, currentDocumentId) {
        for (const doc of documents) {
          const card = document.createElement('div');
          card.className = 'doc-card' + (doc.id === currentDocumentId ? ' current' : '');
          const text = document.createElement('div');
          const title = document.createElement('div');
          title.className = 'doc-title';
          title.textContent = doc.originalName || doc.title;
          const meta = document.createElement('div');
          meta.className = 'doc-meta';
          meta.textContent = doc.currentVersionId + ' · ' + (doc.versions?.length || 0) + ' version(s)';
          text.append(title, meta);
          const actions = document.createElement('div');
          actions.className = 'doc-actions';
          const select = document.createElement('button');
          select.type = 'button';
          select.className = 'secondary';
          select.textContent = doc.id === currentDocumentId ? 'Selected' : 'Select';
          select.dataset.baseDisabled = String(doc.id === currentDocumentId);
          select.disabled = state.busy || doc.id === currentDocumentId;
          select.addEventListener('click', () => selectDocument(doc.id));
          const remove = document.createElement('button');
          remove.type = 'button';
          remove.className = 'danger';
          remove.textContent = 'Remove';
          remove.dataset.baseDisabled = String(documents.length <= 1);
          remove.disabled = state.busy || documents.length <= 1;
          remove.addEventListener('click', () => removeDocument(doc.id, doc.originalName || doc.title));
          actions.append(select, remove);
          card.append(text, actions);
          documentList.append(card);
        }
      }

      function renderPreview(thread, currentDocument) {
	        const versions = currentDocument?.versions || thread.versions || [];
	        const current = versions.find((version) => version.id === thread.currentVersionId);
	        const thumbs = current?.render?.thumbnails || [];
	        const supported = canRenderThread(thread);
	        const ext = (thread.currentExtension || currentDocument?.currentExtension || current?.extension || '').replace('.', '').toUpperCase();
	        preview.innerHTML = '';
	        if (!thumbs.length) {
	          previewMeta.textContent = supported ? 'No rendered thumbnails yet.' : 'Preview unavailable for ' + (ext || 'this file') + '.';
	          preview.innerHTML = supported
	            ? '<div class="empty">' + ${JSON.stringify(previewRenderPromptCopy)} + '</div>'
	            : '<div class="empty">' + ${JSON.stringify(previewWiredOnlyCopy)} + ' You can still inspect, edit, validate, and download this file.</div>';
	          return;
	        }
        previewMeta.textContent = thumbs.length + ' thumbnail(s) · ' + thread.currentVersionId;
        const wrap = document.createElement('div');
        wrap.className = 'thumbs';
        for (const thumb of thumbs) {
          const card = document.createElement('div');
          card.className = 'thumb';
          const img = document.createElement('img');
          img.src = appUrl(thumb.url);
          img.alt = 'Slide ' + thumb.index;
          const label = document.createElement('div');
          label.className = 'doc-meta';
          label.textContent = 'Slide ' + thumb.index;
          card.append(img, label);
          wrap.append(card);
        }
        preview.append(wrap);
      }

      async function selectDocument(documentId) {
        if (!state.thread) return;
        setBusy(true);
        try {
          const response = await apiFetch('/api/threads/' + encodeURIComponent(state.thread.id) + '/documents/' + encodeURIComponent(documentId) + '/select', {
            method: 'POST'
          });
          const data = await readApiJson(response, 'Select file');
          state.thread = data;
          await loadThreads(data.id, false);
          renderThread();
        } catch (error) {
          addMessage('error', error.message || String(error));
        } finally {
          setBusy(false);
        }
      }

      async function removeDocument(documentId, name) {
        if (!state.thread) return;
        if (!confirm('Remove ' + name + ' from this thread?')) return;
        setBusy(true);
        try {
          const response = await apiFetch('/api/threads/' + encodeURIComponent(state.thread.id) + '/documents/' + encodeURIComponent(documentId), {
            method: 'DELETE'
          });
          const data = await readApiJson(response, 'Remove file');
          state.thread = data;
          await loadThreads(data.id, false);
          renderThread();
        } catch (error) {
          addMessage('error', error.message || String(error));
        } finally {
          setBusy(false);
        }
      }

      function addMessage(role, text) {
        if (role === 'trace') {
          return logActivity(text);
        }
        const node = document.createElement('div');
        node.className = 'message ' + role;
        if (role === 'assistant') {
          node.innerHTML = renderMarkdown(text);
        } else {
          node.textContent = text;
        }
        chat.append(node);
        chat.scrollTop = chat.scrollHeight;
        if (role === 'error') logActivity(text, 'error');
        return node;
      }

      function resetActivity(text) {
        state.activityLines = [];
        activityLog.innerHTML = '';
        logActivity(text);
      }

      function logActivity(text, level = 'info') {
        const item = {
          time: new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' }),
          text: String(text || ''),
          level,
        };
        state.activityLines.push(item);
        // Append a single new line so the role=log / aria-live=polite /
        // aria-relevant=additions region announces only the new entry, instead
        // of re-reading the whole buffer (which the full innerHTML rebuild caused).
        const line = document.createElement('div');
        line.className = 'activity-line' + (item.level === 'error' ? ' error' : '');
        const time = document.createElement('span');
        time.className = 'activity-time';
        time.textContent = item.time;
        const body = document.createElement('span');
        body.className = 'activity-text';
        body.title = item.text;
        body.textContent = item.text;
        line.append(time, body);
        activityLog.append(line);
        while (state.activityLines.length > 80) {
          state.activityLines.shift();
          if (activityLog.firstChild) activityLog.removeChild(activityLog.firstChild);
        }
        activityLog.scrollTop = activityLog.scrollHeight;
        return line;
      }

	      async function streamAgentEvents(admission) {
	        const streamUrl = admission?.streamUrl;
	        const offset = admission?.offset;
	        if (!streamUrl || offset === undefined || offset === null) {
	          addMessage('assistant', extractAgentText(admission));
	          return;
	        }
	        addMessage('trace', 'accepted · opening event stream');
	        const assistantNode = addMessage('assistant', '');
	        let assistantText = '';
	        let sawEvent = false;
	        await new Promise((resolve, reject) => {
	          const url = normalizedEventStreamUrl(streamUrl);
	          url.searchParams.set('offset', offset);
	          url.searchParams.set('live', 'sse');
	          const source = new EventSource(url.toString());
          source.onopen = () => addMessage('trace', 'event stream connected');
	          let settled = false;
	          let lastEventAt = Date.now();
	          const watchdog = setInterval(() => {
	            if (settled) return;
	            const idleMs = Date.now() - lastEventAt;
	            if (idleMs > 90_000) {
	              source.close();
	              addMessage('trace', 'event stream timed out after 90s · refreshing thread state');
	              finish();
	            }
	          }, 5_000);
		          const finish = () => {
		            if (settled) return;
		            settled = true;
		            clearInterval(watchdog);
		            state.stopStream = null;
		            source.close();
		            resolve();
		          };
		          state.stopStream = () => {
		            if (settled) return;
		            addMessage('trace', 'stopped · refreshing thread state');
		            finish();
		          };
	          source.addEventListener('data', (event) => {
	            lastEventAt = Date.now();
	            let events;
	            try {
	              const parsed = JSON.parse(event.data);
	              events = Array.isArray(parsed) ? parsed : [parsed];
	            } catch (error) {
	              addMessage('trace', 'trace parse failed · ' + (error.message || String(error)));
	              return;
	            }
	            for (const item of events) {
	              sawEvent = true;
	              handleAgentEvent(item);
	              if (item?.type === 'idle') finish();
	            }
	          });
	          source.addEventListener('control', (event) => {
	            lastEventAt = Date.now();
	            try {
              const control = JSON.parse(event.data);
              if (control.streamClosed) finish();
            } catch {}
          });
	          source.onerror = () => {
	            if (!settled) {
	              source.close();
	              if (sawEvent || assistantText.trim()) {
	                addMessage('trace', 'event stream closed early · refreshing thread state');
	                finish();
	              } else {
		                settled = true;
		                clearInterval(watchdog);
		                state.stopStream = null;
		                reject(new Error('Agent event stream disconnected before any events arrived.'));
	              }
	            }
	          };
	        });
        if (!assistantText.trim()) assistantNode.textContent = '(no assistant text returned)';

        function handleAgentEvent(event) {
          switch (event?.type) {
            case 'operation_start': addMessage('trace', 'operation started'); break;
            case 'agent_start': addMessage('trace', 'agent started'); break;
            case 'turn_start': break;
            case 'tool_start': addMessage('trace', 'tool started · ' + (event.toolName || 'unknown')); break;
            case 'tool':
            case 'tool_call': addMessage('trace', toolTraceText(event)); break;
            case 'text_delta':
              assistantText += event.text || '';
              assistantNode.innerHTML = renderMarkdown(assistantText);
              chat.scrollTop = chat.scrollHeight;
              break;
            case 'message_end':
              if (!assistantText && typeof event.message?.content?.[0]?.text === 'string') {
                assistantText = event.message.content[0].text;
                assistantNode.innerHTML = renderMarkdown(assistantText);
              }
              break;
            case 'operation':
              if (event.isError || event.error) {
                addMessage('trace', 'operation failed · ' + readableError(event.error));
                addMessage('error', 'Agent operation failed · ' + readableError(event.error));
              } else {
                if (typeof event.result?.text === 'string') {
                  assistantText = event.result.text;
                  assistantNode.innerHTML = renderMarkdown(assistantText);
                }
                const usage = event.result?.usage;
                addMessage('trace', usage?.totalTokens ? 'operation finished · ' + usage.totalTokens + ' tokens' + costText(usage) : 'operation finished');
              }
              break;
            case 'idle': addMessage('trace', 'done'); break;
          }
        }
      }

      function renderMarkdown(text) {
        const escaped = escapeHtml(text || '');
        const lines = escaped.split('\\n');
        let html = '';
        let inList = false;
        for (const line of lines) {
          if (/^\\s*-\\s+/.test(line)) {
            if (!inList) { html += '<ul>'; inList = true; }
            html += '<li>' + inlineMarkdown(line.replace(/^\\s*-\\s+/, '')) + '</li>';
          } else {
            if (inList) { html += '</ul>'; inList = false; }
            if (line.trim()) html += '<p>' + inlineMarkdown(line) + '</p>';
          }
        }
        if (inList) html += '</ul>';
        return html || '<p></p>';
      }

      function inlineMarkdown(text) {
        return text
          .replace(/\`([^\`]+)\`/g, '<code>$1</code>')
          .replace(/\\*\\*([^*]+)\\*\\*/g, '<strong>$1</strong>')
          .replace(/(\\/api\\/[^\\s<]+)/g, (match) => '<a href="' + appUrl(match) + '">' + match + '</a>');
      }

      function escapeHtml(value) {
        return String(value)
          .replace(/&/g, '&amp;')
          .replace(/</g, '&lt;')
          .replace(/>/g, '&gt;')
          .replace(/"/g, '&quot;')
          .replace(/'/g, '&#039;');
      }

      function toolTraceText(event) {
        const status = event.isError || event.error ? 'failed' : 'finished';
        const duration = typeof event.durationMs === 'number' ? ' · ' + event.durationMs + 'ms' : '';
        const error = event.error ? ' · ' + readableError(event.error) : '';
        return 'tool ' + status + ' · ' + (event.toolName || 'unknown') + duration + error;
      }

      function readableError(error) {
        if (!error) return 'unknown error';
        if (typeof error === 'string') return error;
        return error.message || error.name || JSON.stringify(error);
      }

      function costText(usage) {
        const total = usage?.cost?.total;
        return typeof total === 'number' ? ' · $' + total.toFixed(4) : '';
      }

	      function setBusy(isBusy, label) {
	        state.busy = isBusy;
	        if (label) state.busyLabel = label;
		        uploadBtn.disabled = isBusy;
		        promptInput.disabled = isBusy || !state.thread;
		        sendBtn.disabled = isBusy || !state.thread;
		        stopBtn.hidden = !isBusy;
		        refreshBtn.disabled = isBusy || !state.thread;
	        renderBtn.disabled = isBusy || !state.thread || !canRenderCurrent();
        newThreadBtn.disabled = isBusy;
        documentList.querySelectorAll('button').forEach((button) => {
          button.disabled = isBusy || button.dataset.baseDisabled === 'true';
        });
	        updateStatus();
	      }

	      function setZoom(value) {
	        state.thumbWidth = value;
	        zoomRange.value = String(value);
	        document.documentElement.style.setProperty('--thumb-width', value + 'px');
	      }

	      function canRenderCurrent() {
	        return canRenderThread(state.thread);
	      }

	      function canRenderThread(thread) {
	        return Boolean(thread?.previewSupported);
	      }

	      function updateStatus() {
	        if (state.busy) {
	          statusText.textContent = state.busyLabel;
	          statusDot.classList.add('running');
	          return;
	        }
	        statusDot.classList.remove('running');
	        if (!state.thread) {
	          statusText.textContent = 'Upload a file to begin.';
	        } else {
	          statusText.textContent = canRenderCurrent() ? 'Ready. Preview supported.' : 'Ready. Preview not available.';
	        }
	      }

      function agentErrorMessage(data) {
        const raw = data?.error?.details || data?.error?.message || data?.error;
        if (raw === 'An internal error occurred.' || raw === 'The server encountered an unexpected error while handling this request.') {
          return 'Agent request failed. Check the server log for the request error.';
        }
        return raw || 'Agent request failed';
      }

      function extractAgentText(data) {
        if (typeof data?.result?.text === 'string') return data.result.text;
        if (typeof data?.text === 'string') return data.text;
        if (typeof data?.result === 'string') return data.result;
        return JSON.stringify(data, null, 2);
      }

      async function readApiJson(response, label, errorFormatter) {
        const contentType = response.headers.get('content-type') || '';
        const text = await response.text();
        let data = {};
        if (text) {
          const jsonLike = contentType.includes('application/json') || /^\\s*[\\[{]/.test(text);
          if (jsonLike) {
            try {
              data = JSON.parse(text);
            } catch (error) {
              throw new Error(label + ' returned invalid JSON (' + response.status + ') from ' + responsePath(response) + '.');
            }
          } else {
            throw new Error(nonJsonResponseMessage(response, label, contentType, text));
          }
        }
        if (!response.ok) {
          const formatted = typeof errorFormatter === 'function' ? errorFormatter(data) : undefined;
          throw new Error(formatted || data?.error?.message || data?.error || data?.message || label + ' failed (' + response.status + ').');
        }
        return data;
      }

      function nonJsonResponseMessage(response, label, contentType, text) {
        const type = contentType || 'no content type';
        const path = responsePath(response);
        if (/^\\s*<!doctype|^\\s*<html/i.test(text)) {
          return label + ' returned an HTML page instead of JSON (' + response.status + ', ' + type + ') from ' + path + '. Refresh the page and sign in again; if it repeats, this is a proxy/auth routing bug.';
        }
        const preview = text.replace(/\\s+/g, ' ').slice(0, 120);
        return label + ' returned non-JSON (' + response.status + ', ' + type + ') from ' + path + ': ' + preview;
      }

      function responsePath(response) {
        try {
          const url = new URL(response.url);
          return url.pathname + url.search;
        } catch {
          return 'the requested endpoint';
        }
      }

      function normalizedEventStreamUrl(value) {
        const parsed = new URL(String(value || ''), window.location.origin);
        const unprefixedPath = removeAppBasePath(parsed.pathname);
        let streamPath = '';
        if (unprefixedPath.startsWith('/flue/')) {
          streamPath = unprefixedPath;
        } else if (unprefixedPath.startsWith('/agents/') || unprefixedPath.startsWith('/runs/')) {
          streamPath = '/flue' + unprefixedPath;
        }
        if (!streamPath) {
          throw new Error('Agent returned an unexpected event stream URL: ' + parsed.pathname);
        }
        return new URL(appUrl(streamPath + parsed.search + parsed.hash), window.location.origin);
      }

      function removeAppBasePath(pathname) {
        if (!APP_BASE_PATH) return pathname;
        if (pathname === APP_BASE_PATH) return '/';
        if (pathname.startsWith(APP_BASE_PATH + '/')) return pathname.slice(APP_BASE_PATH.length) || '/';
        return pathname;
      }

      async function apiFetch(url, options = {}) {
        const init = { ...options };
        const method = String(init.method || 'GET').toUpperCase();
        const headers = new Headers(init.headers || {});
        if (!headers.has('accept')) headers.set('accept', 'application/json');
        if (!['GET', 'HEAD', 'OPTIONS'].includes(method)) {
          const csrf = cookieValue('ooxml_csrf') || state.csrfToken || await refreshCsrfToken();
          if (csrf) headers.set('x-ooxml-csrf', csrf);
        }
        init.headers = headers;
        const response = await fetch(appUrl(url), init);
        if (response.status === 401) {
          window.location.href = appUrl('/signin?returnTo=' + encodeURIComponent(window.location.pathname + window.location.search));
        }
        return response;
      }

      async function refreshCsrfToken() {
        try {
          const response = await fetch(appUrl('/api/auth/me'), {
            headers: { accept: 'application/json' }
          });
          if (!response.ok) return '';
          const data = await response.json().catch(() => ({}));
          if (data?.csrfToken) state.csrfToken = data.csrfToken;
          return state.csrfToken || cookieValue('ooxml_csrf');
        } catch {
          return '';
        }
      }

      function appUrl(value) {
        const url = String(value || '/');
        if (/^https?:\\/\\//i.test(url)) {
          const parsed = new URL(url);
          if (parsed.origin !== window.location.origin) return url;
          return parsed.pathname.startsWith(APP_BASE_PATH + '/') || parsed.pathname === APP_BASE_PATH
            ? parsed.pathname + parsed.search + parsed.hash
            : prefixPath(parsed.pathname + parsed.search + parsed.hash);
        }
        if (!APP_BASE_PATH) return url;
        if (url === APP_BASE_PATH || url.startsWith(APP_BASE_PATH + '/') || url.startsWith(APP_BASE_PATH + '?')) return url;
        if (url.startsWith('/')) return prefixPath(url);
        return prefixPath('/' + url);
      }

      function prefixPath(path) {
        if (!APP_BASE_PATH) return path;
        return path === '/' ? APP_BASE_PATH : APP_BASE_PATH + path;
      }

      function isAppPath(pathname, innerPath) {
        const expected = appUrl(innerPath);
        return pathname === expected || pathname.startsWith(expected);
      }

      function cookieValue(name) {
        const prefix = name + '=';
        const match = document.cookie
          .split(';')
          .map((part) => part.trim())
          .find((part) => part.startsWith(prefix));
        return match ? decodeURIComponent(match.slice(prefix.length)) : '';
      }
    </script>
  </body>
</html>`;
}
