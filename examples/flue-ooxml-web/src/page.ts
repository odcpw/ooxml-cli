export function workbenchHtml(): string {
  return `<!doctype html>
<html lang="en" class="dark">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>OOXML Agent Workbench</title>
    <style>
      :root {
        color-scheme: dark;
        --color-bg: #0e0e10;
        --color-surface: #16161a;
        --color-surface-elev: #1c1c21;
        --color-border: #2a2a32;
        --color-text: #e4e4e8;
        --color-muted: #8e8e9a;
        --color-accent: #7b83ff;
        --color-accent-2: #34d399;
        --color-danger: #f87171;
        --font-sans: Inter, ui-sans-serif, system-ui, -apple-system, "Segoe UI", sans-serif;
        --font-mono: "JetBrains Mono", ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
        --thumb-width: 280px;
      }
      * { box-sizing: border-box; }
	      html, body { margin: 0; min-height: 100%; overflow: hidden; }
	      body {
	        min-height: 100vh;
        color: var(--color-text);
        background: var(--color-bg);
        font-family: var(--font-sans);
        -webkit-font-smoothing: antialiased;
      }
      button, input, textarea { font: inherit; }
      button {
        border: 1px solid transparent;
        border-radius: 7px;
        padding: 8px 10px;
        background: var(--color-accent);
        color: #fff;
        font-weight: 650;
        cursor: pointer;
      }
      button.secondary {
        background: #202028;
        border-color: var(--color-border);
        color: var(--color-text);
      }
      button.ghost {
        background: transparent;
        border-color: transparent;
        color: var(--color-muted);
      }
      button.danger {
        background: transparent;
        border-color: #4a2228;
        color: var(--color-danger);
      }
      button:disabled { opacity: .5; cursor: not-allowed; }
      input[type="file"], input[type="text"], textarea {
        width: 100%;
        border: 1px solid var(--color-border);
        border-radius: 7px;
        background: #111115;
        color: var(--color-text);
        padding: 9px 10px;
      }
      input[type="range"] { width: 150px; accent-color: var(--color-accent); }
      textarea { min-height: 90px; resize: vertical; }
	      .app {
	        display: grid;
	        grid-template-columns: 280px minmax(360px, 520px) minmax(0, 1fr);
	        height: 100vh;
	        overflow: hidden;
	      }
	      .pane {
	        min-height: 0;
	        border-right: 1px solid var(--color-border);
	        background: var(--color-surface);
	      }
	      .threads {
	        display: grid;
	        grid-template-rows: auto 1fr auto;
	        min-height: 0;
	      }
      .pane-head {
        padding: 16px;
        border-bottom: 1px solid var(--color-border);
      }
      .brand {
        font-size: 15px;
        font-weight: 750;
        letter-spacing: 0;
      }
      .subtle { color: var(--color-muted); font-size: 12px; line-height: 1.35; }
      .thread-list, .doc-list, .chat-log { overflow: auto; }
      .thread-list { padding: 10px; }
      .thread-row {
        width: 100%;
        display: block;
        text-align: left;
        border: 1px solid transparent;
        background: transparent;
        color: var(--color-text);
        padding: 10px;
        border-radius: 8px;
        margin-bottom: 6px;
      }
      .thread-row:hover, .thread-row.current {
        background: var(--color-surface-elev);
        border-color: var(--color-border);
      }
      .thread-title, .doc-title {
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
        font-size: 13px;
        font-weight: 650;
      }
      .thread-meta, .doc-meta {
        margin-top: 3px;
        color: var(--color-muted);
        font-size: 11px;
      }
	      .work {
	        display: grid;
	        grid-template-rows: auto auto 1fr auto;
	        min-height: 0;
	      }
      .section { padding: 14px; border-bottom: 1px solid var(--color-border); }
      .account-strip {
        display: grid;
        grid-template-columns: minmax(0, 1fr) auto;
        gap: 8px;
        align-items: center;
        margin-top: 10px;
        padding-top: 10px;
        border-top: 1px solid var(--color-border);
      }
      .account-strip button { padding: 6px 8px; font-size: 12px; }
      .section-title {
        margin: 0 0 10px;
        font-size: 12px;
        color: var(--color-muted);
        text-transform: uppercase;
        letter-spacing: .08em;
      }
	      .upload-form { display: grid; gap: 9px; }
	      .row { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; }
	      .composer-row { justify-content: space-between; }
	      .status-line {
	        display: inline-flex;
	        align-items: center;
	        gap: 7px;
	        color: var(--color-muted);
	        font-size: 12px;
	        min-width: 150px;
	      }
	      .status-dot {
	        width: 7px;
	        height: 7px;
	        border-radius: 999px;
	        background: #3f3f46;
	      }
	      .status-dot.running {
	        background: var(--color-accent-2);
	        box-shadow: 0 0 0 4px rgba(52, 211, 153, .12);
	      }
      .doc-list { max-height: 260px; display: grid; gap: 8px; }
      .doc-card {
        display: grid;
        grid-template-columns: minmax(0, 1fr) auto;
        gap: 8px;
        align-items: center;
        border: 1px solid var(--color-border);
        border-radius: 8px;
        padding: 9px;
        background: #121217;
      }
      .doc-card.current {
        border-color: #4f56c4;
        background: #17182a;
      }
      .doc-actions { display: flex; gap: 6px; align-items: center; }
      .doc-actions button { padding: 6px 8px; font-size: 12px; }
      .chat-log {
        padding: 14px;
        display: flex;
        flex-direction: column;
        gap: 10px;
      }
      .message {
        border: 1px solid var(--color-border);
        border-radius: 8px;
        background: #121217;
        padding: 10px 11px;
        font-size: 13px;
        line-height: 1.45;
        white-space: normal;
      }
      .message.user {
        background: #10251d;
        border-color: #1f6a4a;
      }
      .message.assistant {
        background: #15151b;
      }
      .message.trace {
        background: #111827;
        border-color: #263244;
        color: #a6adbb;
        font-size: 12px;
        padding: 7px 9px;
      }
      .message.error {
        background: #2a1215;
        border-color: #6b2630;
        color: #fecaca;
      }
      .message p { margin: 0 0 8px; }
      .message p:last-child { margin-bottom: 0; }
      .message ul { margin: 6px 0 8px 18px; padding: 0; }
      .message code {
        font-family: var(--font-mono);
        background: #23232b;
        border: 1px solid #30303a;
        border-radius: 5px;
        padding: 1px 4px;
        font-size: 12px;
      }
      .composer {
        padding: 14px;
        border-top: 1px solid var(--color-border);
        background: var(--color-surface);
      }
	      .preview-pane {
	        min-height: 0;
	        background: #0b0b0d;
	        display: grid;
	        grid-template-rows: auto 1fr;
	      }
      .preview-head {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 14px;
        padding: 14px 16px;
        border-bottom: 1px solid var(--color-border);
        background: var(--color-surface);
      }
      .preview-title { font-size: 14px; font-weight: 700; }
	      .preview-body {
	        overflow: auto;
	        padding: 16px;
	        min-height: 0;
	      }
	      .thumbs {
	        display: grid;
	        grid-template-columns: repeat(auto-fill, minmax(min(var(--thumb-width), 100%), var(--thumb-width)));
	        gap: 16px;
	        align-items: start;
	        justify-content: start;
	      }
      .thumb {
        border: 1px solid var(--color-border);
        border-radius: 8px;
        padding: 8px;
        background: var(--color-surface);
      }
      .thumb img {
        display: block;
        width: 100%;
        height: auto;
        border-radius: 5px;
        background: #050507;
      }
      .empty {
        color: var(--color-muted);
        border: 1px dashed var(--color-border);
        border-radius: 8px;
        padding: 16px;
      }
      a { color: #aab0ff; }
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
            <input id="titleInput" type="text" placeholder="Thread title" />
            <input id="fileInput" type="file" accept=".pptx,.pptm,.docx,.xlsx,.xlsm" multiple required />
            <button id="uploadBtn" type="submit">Upload file(s)</button>
          </form>
        </div>
        <div class="section">
          <h2 class="section-title">Library</h2>
          <div id="threadInfo" class="subtle">No thread selected.</div>
          <div id="documentList" class="doc-list"></div>
          <div class="row" style="margin-top:10px">
            <button id="refreshBtn" class="secondary" disabled>Refresh</button>
            <button id="renderBtn" class="secondary" disabled>Render preview</button>
            <a id="downloadLink" href="#" hidden>Download current</a>
          </div>
        </div>
        <div id="chat" class="chat-log"></div>
        <form id="chatForm" class="composer">
	          <textarea id="promptInput" placeholder="Ask the agent to translate slides, inspect, validate, render, search, or make exact text changes..." disabled></textarea>
	          <div class="row composer-row" style="margin-top:8px">
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
            <div id="previewMeta" class="subtle">Render PPTX/PPTM thumbnails to inspect output.</div>
          </div>
          <div class="row">
            <button id="zoomOutBtn" class="secondary" type="button">-</button>
            <input id="zoomRange" type="range" min="180" max="720" step="20" value="280" />
            <button id="zoomInBtn" class="secondary" type="button">+</button>
          </div>
        </div>
        <div id="preview" class="preview-body"></div>
      </main>
    </div>
    <script>
		      const state = { threads: [], thread: null, thumbWidth: 280, busy: false, busyLabel: 'Working', stopStream: null };
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
      const previewMeta = document.getElementById('previewMeta');
      const downloadLink = document.getElementById('downloadLink');
	      const zoomRange = document.getElementById('zoomRange');
	      const zoomOutBtn = document.getElementById('zoomOutBtn');
	      const zoomInBtn = document.getElementById('zoomInBtn');
	      const statusDot = document.getElementById('statusDot');
	      const statusText = document.getElementById('statusText');

	      loadAccount().catch(() => undefined);
	      loadThreads().catch((error) => {
	        addMessage('error', error.message || String(error));
	        updateStatus();
	      });

      logoutBtn.addEventListener('click', async () => {
        await apiFetch('/api/auth/logout', { method: 'POST' }).catch(() => undefined);
        window.location.href = '/signin';
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
          const data = await response.json();
          if (!response.ok) throw new Error(data.error || 'Upload failed');
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
	          addMessage('trace', 'preview skipped · PPTX/PPTM thumbnails only');
	          return;
	        }
	        addMessage('trace', 'rendering preview');
	        setBusy(true, 'Rendering preview');
	        try {
	          const response = await apiFetch('/api/threads/' + encodeURIComponent(state.thread.id) + '/render', { method: 'POST' });
	          const data = await response.json();
	          if (!response.ok) throw new Error(data.error || 'Render failed');
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
        promptInput.value = '';
        addMessage('user', message);
	        setBusy(true, 'Agent working');
	        try {
          const response = await apiFetch('/flue/agents/ooxml-editor/' + encodeURIComponent(state.thread.id), {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ message })
          });
          const data = await response.json();
          if (!response.ok) throw new Error(agentErrorMessage(data));
          if (data.submissionId) addMessage('trace', 'submission accepted · ' + String(data.submissionId).slice(0, 8));
          await streamAgentEvents(data);
          await refreshThread();
          await loadThreads(state.thread.id, false);
        } catch (error) {
          addMessage('error', error.message || String(error));
        } finally {
          setBusy(false);
        }
      });

      zoomRange.addEventListener('input', () => setZoom(Number(zoomRange.value)));
      zoomOutBtn.addEventListener('click', () => setZoom(Math.max(180, state.thumbWidth - 40)));
      zoomInBtn.addEventListener('click', () => setZoom(Math.min(720, state.thumbWidth + 40)));

      async function loadAccount() {
        const response = await apiFetch('/api/auth/me');
        const data = await response.json();
        if (response.ok && data.user?.email) {
          accountLine.textContent = data.user.email;
        }
      }

      async function loadThreads(selectId, loadSelected = true) {
        const response = await apiFetch('/api/threads');
        const data = await response.json();
        if (!response.ok) throw new Error(data.error || 'Could not load threads');
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
	        const data = await response.json();
	        if (!response.ok) throw new Error(data.error || 'Thread not found');
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
	        renderBtn.title = enabled && !canRenderThread(thread) ? 'Preview thumbnails are currently available for PPTX/PPTM only.' : '';

        if (!thread) {
          threadInfo.textContent = 'No thread selected.';
          preview.innerHTML = '<div class="empty">Upload files or open a thread to begin.</div>';
          previewMeta.textContent = 'Render PPTX/PPTM thumbnails to inspect output.';
	          uploadBtn.textContent = 'Upload file(s)';
	          updateStatus();
	          return;
	        }

        const documents = thread.documents || [];
        const currentDocument = documents.find((document) => document.id === thread.currentDocumentId);
        threadInfo.textContent = thread.title + ' · ' + (thread.currentDocumentName || currentDocument?.originalName || thread.currentFile || '');
        uploadBtn.textContent = 'Add file(s)';
        downloadLink.href = thread.downloadUrl;
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
          select.disabled = doc.id === currentDocumentId;
          select.addEventListener('click', () => selectDocument(doc.id));
          const remove = document.createElement('button');
          remove.type = 'button';
          remove.className = 'danger';
          remove.textContent = 'Remove';
          remove.disabled = documents.length <= 1;
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
	            ? '<div class="empty">Click Render preview for PPTX/PPTM files.</div>'
	            : '<div class="empty">Preview thumbnails are currently wired for PPTX/PPTM. You can still inspect, edit, validate, and download this file.</div>';
	          return;
	        }
        previewMeta.textContent = thumbs.length + ' thumbnail(s) · ' + thread.currentVersionId;
        const wrap = document.createElement('div');
        wrap.className = 'thumbs';
        for (const thumb of thumbs) {
          const card = document.createElement('div');
          card.className = 'thumb';
          const img = document.createElement('img');
          img.src = thumb.url;
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
          const data = await response.json();
          if (!response.ok) throw new Error(data.error || 'Select failed');
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
          const data = await response.json();
          if (!response.ok) throw new Error(data.error || 'Remove failed');
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
        const node = document.createElement('div');
        node.className = 'message ' + role;
        if (role === 'assistant') {
          node.innerHTML = renderMarkdown(text);
        } else {
          node.textContent = text;
        }
        chat.append(node);
        chat.scrollTop = chat.scrollHeight;
        return node;
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
	          const url = new URL(streamUrl, window.location.origin);
	          if (url.origin !== window.location.origin || !url.pathname.startsWith('/flue/')) {
	            throw new Error('Agent returned an unexpected event stream URL.');
	          }
	          url.searchParams.set('offset', offset);
	          url.searchParams.set('live', 'sse');
	          const source = new EventSource(url.toString());
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
          .replace(/(\\/api\\/[^\\s<]+)/g, '<a href="$1">$1</a>');
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

      async function apiFetch(url, options = {}) {
        const init = { ...options };
        const method = String(init.method || 'GET').toUpperCase();
        const headers = new Headers(init.headers || {});
        if (!['GET', 'HEAD', 'OPTIONS'].includes(method)) {
          const csrf = cookieValue('ooxml_csrf');
          if (csrf) headers.set('x-ooxml-csrf', csrf);
        }
        init.headers = headers;
        const response = await fetch(url, init);
        if (response.status === 401) {
          window.location.href = '/signin?returnTo=' + encodeURIComponent(window.location.pathname + window.location.search);
        }
        return response;
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
