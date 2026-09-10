import { appPathPrefix } from './shared/app-url.ts';
import { themeCss } from './shared/theme.ts';

export function workbenchHtml(): string {
  const basePath = appPathPrefix();
  return `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Slides · SafetySecretary</title>
  <style>
${themeCss()}
    :root { --color-accent:#4655ae; --color-muted:#595e6c; --color-bg:#f3f4f7; --color-surface:#fff; }
    * { box-sizing:border-box; }
    body { margin:0; background:var(--color-bg); font-size:14px; }
    button,input,select,textarea { font:inherit; }
    button,.download { min-height:40px; border:1px solid var(--color-border); border-radius:8px; padding:8px 14px; cursor:pointer; background:#fff; color:var(--color-text); }
    button:hover:not(:disabled) { border-color:var(--color-accent); background:#f2f3fb; }
    button:disabled { opacity:.5; cursor:default; }
    button.primary,.download { background:var(--color-accent); color:#fff; border-color:var(--color-accent); font-weight:600; text-decoration:none; }
    button.primary:hover:not(:disabled) { background:#394795; }
    [hidden] { display:none!important; }
    input[type=text],select,textarea { width:100%; min-width:0; border:1px solid #ced1db; background:#fff; color:var(--color-text); border-radius:7px; padding:9px 10px; }
    textarea { resize:vertical; line-height:1.5; }
    input[type=file] { max-width:100%; font-size:12px; }
    input::file-selector-button { border:1px solid #ced1db; border-radius:6px; background:#fff; padding:7px 9px; margin-right:8px; cursor:pointer; }
    :focus-visible { outline:3px solid #8994e0; outline-offset:3px; }
    label,.field-label { display:block; font-weight:600; font-size:13px; margin-bottom:6px; }
    h1,h2,p { margin:0; } h1 { font-size:19px; letter-spacing:-.5px; } h2 { font-size:16px; }
    .subtle { color:var(--color-muted); font-size:12px; line-height:1.5; }
    .topbar { height:72px; padding:12px 28px; background:white; border-bottom:1px solid var(--color-border); display:flex; align-items:center; justify-content:space-between; gap:16px; }
    .row { display:flex; align-items:center; gap:10px; flex-wrap:wrap; }
    .recent { position:relative; }
    summary { cursor:pointer; font-size:13px; padding:8px 0; }
    .recent .thread-list { position:absolute; z-index:5; right:0; top:40px; width:300px; max-height:360px; overflow:auto; padding:10px; border:1px solid var(--color-border); background:white; border-radius:10px; box-shadow:0 10px 30px #19204420; }
    .thread-row { display:block; width:100%; text-align:left; margin:4px 0; }
    .thread-title { overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
    .app { display:grid; grid-template-columns:400px minmax(0,1fr); height:calc(100dvh - 72px); max-width:1800px; margin:auto; }
    .work { min-height:0; display:flex; flex-direction:column; background:white; border-right:1px solid var(--color-border); }
    .setup { padding:20px; overflow:auto; max-height:55%; flex-shrink:0; border-bottom:1px solid var(--color-border); }
    .setup > summary { font-weight:600; padding:0 0 10px; }
    .setup:not([open]) { padding-bottom:10px; }
    .task-switch { display:grid; grid-template-columns:1fr 1fr; gap:8px; margin:6px 0 18px; }
    .task-switch button[aria-pressed=true] { background:#eef0fc; border-color:var(--color-accent); color:#35438f; font-weight:600; }
    .upload-slot { border:1px solid var(--color-border); padding:12px; border-radius:9px; margin:10px 0; background:#fcfcfe; }
    .upload-slot .subtle { margin:4px 0 8px; }
    .upload-slot select { margin-bottom:8px; }
    .field { margin-top:14px; }
    .optional { font-weight:400; color:var(--color-muted); }
    #setupSummary { display:inline; }
    .setup-hint { margin-top:12px; }
    #chat { flex:1; min-height:100px; overflow:auto; padding:18px 20px; }
    .chat-empty { color:var(--color-muted); line-height:1.6; font-size:13px; }
    .message { line-height:1.6; margin-bottom:14px; overflow-wrap:anywhere; }
    .message.user { background:#eef0fc; border-radius:10px; padding:10px 12px; }
    .message.assistant { padding:4px 0; }
    .message.error { background:#fff0ed; border-left:3px solid #b64332; padding:10px; color:#913626; }
    .message p { margin:0 0 8px; } .message ul,.message ol { padding-left:22px; }
    .message pre { overflow:auto; background:#f5f5f8; padding:10px; } .message table { display:block; overflow:auto; }
    .composer { padding:14px 20px 18px; border-top:1px solid var(--color-border); }
    .composer textarea { min-height:80px; max-height:200px; }
    .composer-row { justify-content:space-between; margin-top:10px; }
    .status-line { display:flex; align-items:center; gap:7px; font-size:12px; color:var(--color-muted); margin-top:10px; }
    .status-dot { width:7px; height:7px; border-radius:50%; background:#5b826e; }
    .status-dot.running { background:#b8852a; animation:pulse 1.5s infinite; }
    @keyframes pulse { 50% { opacity:.35; } }
    @media(prefers-reduced-motion:reduce) { .status-dot.running { animation:none; } }
    .activity-log { max-height:90px; overflow:auto; font:11px/1.5 monospace; color:var(--color-muted); }
    .activity-line { display:flex; gap:8px; } .activity-time { flex-shrink:0; }
    .activity-text { overflow-wrap:anywhere; } .activity-line.error { color:#913626; }
    .preview-pane { min-width:0; min-height:0; display:flex; flex-direction:column; padding:24px 28px; gap:18px; }
    .preview-head { display:flex; align-items:flex-start; justify-content:space-between; gap:16px; }
    .preview-head h2 { margin-bottom:3px; }
    .preview-controls { display:flex; gap:8px; flex-wrap:wrap; align-items:center; }
    .preview-controls select { width:auto; max-width:260px; }
    .preview-body { min-height:0; flex:1; display:flex; flex-direction:column; justify-content:center; overflow:auto; }
    .empty { text-align:center; color:var(--color-muted); padding:30px; line-height:1.7; }
    .empty h2 { font-size:22px; color:#373d50; margin-bottom:10px; }
    .empty ol { text-align:left; max-width:310px; margin:22px auto 0; padding-left:23px; } .empty li { padding:5px 0; }
    .slide-frame { flex:1; display:flex; align-items:center; justify-content:center; width:100%; min-height:0; text-align:center; }
    .slide-frame img { display:block; max-width:100%; max-height:calc(100dvh - 340px); width:auto; height:auto; margin:auto; box-shadow:0 6px 28px #222b461a; background:white; }
    .slide-nav { flex-shrink:0; display:flex; justify-content:center; align-items:center; gap:16px; padding-top:18px; }
    .slide-nav button { min-width:44px; }
    .preview-foot { display:flex; align-items:center; justify-content:space-between; gap:12px; }
    #documentList { display:grid; gap:6px; margin-top:8px; }
    .doc-card { display:flex; align-items:center; justify-content:space-between; gap:8px; padding:6px 0; font-size:12px; }
    .doc-card span { overflow-wrap:anywhere; } .doc-card button { font-size:12px; padding:4px 8px; min-height:30px; }
    .file-list { font-size:12px; color:var(--color-muted); margin-top:7px; overflow-wrap:anywhere; }
    @media(max-width:1050px) { .app { grid-template-columns:360px minmax(0,1fr); } .preview-pane { padding:20px; } .topbar { padding:12px 20px; } }
    @media(max-width:760px) {
      .topbar { height:auto; min-height:72px; padding:12px 16px; flex-wrap:wrap; } .topbar .subtle { display:none; }
      .app { display:flex; flex-direction:column; height:auto; } .work { display:contents; }
      .setup { order:0; max-height:none; overflow:visible; padding:18px; background:white; }
      .preview-pane { order:1; min-height:260px; padding:18px; } #chat { order:2; min-height:90px; max-height:380px; background:white; }
      .composer { order:3; background:white; } .slide-frame img { max-height:380px; }
      .preview-head { flex-wrap:wrap; } .preview-controls select { max-width:100%; } .recent .thread-list { right:-80px; width:270px; }
    }
  </style>
</head>
<body>
<header class="topbar">
  <div><h1>SafetySecretary <span style="font-weight:400;color:#747989">/ Slides</span></h1><p class="subtle">Change a template. Translate a presentation.</p></div>
  <div class="row">
    <details class="recent" id="recentWork"><summary>Previous work</summary><div id="threadList" class="thread-list"></div></details>
    <button id="newThreadBtn" type="button">New job</button>
    <button id="logoutBtn" type="button">Sign out</button>
  </div>
</header>
<div class="app">
  <section class="work" aria-label="Files and instructions">
    <details id="setupPanel" class="setup" open>
      <summary><span id="setupSummary">1. Choose your task and files</span></summary>
      <div class="task-switch" role="group" aria-label="What would you like to do?">
        <button id="templateMode" type="button" aria-pressed="true">Change template</button>
        <button id="translateMode" type="button" aria-pressed="false">Translate</button>
      </div>
      <div class="upload-slot">
        <label for="sourceSelect">Deck to change</label>
        <select id="sourceSelect" aria-label="Deck to change" hidden></select>
        <p class="subtle">Your original is kept. Changes are saved as new versions.</p>
        <input id="fileInput" type="file" accept=".pptx,.pptm" multiple aria-label="Upload source decks" />
      </div>
      <div class="upload-slot" id="templateSlot">
        <label for="templateSelect">New template</label>
        <select id="templateSelect" aria-label="New template" hidden></select>
        <p class="subtle">Upload a PowerPoint with the design you want to use.</p>
        <input id="templateInput" type="file" accept=".pptx,.pptm" aria-label="Upload template" />
      </div>
      <div id="translationFields" hidden>
        <div class="field"><label for="languageInput">Translate into</label><input id="languageInput" type="text" value="Italian" placeholder="For example, Italian" maxlength="80" /></div>
        <div class="upload-slot">
          <label for="referenceInput">Reference decks <span class="optional">· optional</span></label>
          <p class="subtle">For example, the French version of your German source deck.</p>
          <input id="referenceInput" type="file" accept=".pptx,.pptm" multiple />
          <div id="referenceList" class="file-list"></div>
        </div>
        <details class="field" id="termsPanel"><summary>Preferred terms <span class="optional">· optional</span></summary>
          <label for="glossaryInput">Terms to use in the translation</label>
          <textarea id="glossaryInput" rows="3" maxlength="50000" placeholder="One term per line, for example:&#10;Arbeitssicherheit → sicurezza sul lavoro"></textarea>
          <label for="glossaryFile" style="margin-top:8px">Or import a TXT / CSV terms list</label>
          <input id="glossaryFile" type="file" accept=".txt,.csv" /><div id="glossaryStatus" class="subtle" role="status"></div>
        </details>
      </div>
      <p id="setupHint" class="subtle setup-hint">Upload your source deck, then the new template.</p>
      <details class="field" id="filesPanel"><summary>Manage uploaded files</summary><div id="documentList"></div></details>
    </details>
    <div id="chat" role="log" aria-label="Conversation"><div class="chat-empty">2. Tell me what you want to change.<br>You can add details now, or use the task above as your starting point.</div></div>
    <form id="chatForm" class="composer">
      <label for="promptInput">Your instructions</label>
      <textarea id="promptInput" placeholder="For example: use the new template, keep all pictures and shorten long titles."></textarea>
      <div class="row composer-row"><button id="sendBtn" class="primary" type="submit" disabled>Change template</button><button id="stopBtn" type="button" hidden>Stop waiting</button></div>
      <div class="status-line" role="status"><span id="statusDot" class="status-dot"></span><span id="statusText">Upload a source deck to begin.</span></div>
      <details><summary class="subtle">Activity details</summary><div id="activityLog" class="activity-log"></div></details>
    </form>
  </section>
  <main class="preview-pane" aria-label="Slide preview">
    <div class="preview-head"><div><h2>Preview</h2><div id="previewMeta" class="subtle">Your slides will appear here.</div></div><a id="downloadLink" class="download" href="#" hidden>Download result</a></div>
    <div class="preview-controls" id="previewControls" hidden>
      <select id="previewDocument" aria-label="File to preview"></select>
      <select id="previewVersion" aria-label="Version to preview"><option value="latest">Latest result</option><option value="original">Original upload</option></select>
      <button id="renderBtn" type="button">Refresh preview</button>
    </div>
    <div id="preview" class="preview-body"><div class="empty"><h2>A fresh version of your slides.</h2><p>Start with the presentation you already have.</p><ol><li>Upload a source deck.</li><li>Add a template or choose a language.</li><li>Describe the changes, then review and download.</li></ol></div></div>
    <div class="preview-foot"><span class="subtle" id="resultHint">Original files stay available.</span><a class="subtle" href="${basePath}/privacy">Privacy</a></div>
  </main>
</div>
<script>
const APP_BASE_PATH = ${JSON.stringify(basePath)};
const state = { threads: [], thread: null, busy: false, busyLabel: '', stopStream: null, csrfToken: '', activityLines: [], previewId: '', previewVersion: 'latest', slide: 0, previewKey: '', attemptedPreview: '', draft: null, followup: false, dirty: false };
const $ = id => document.getElementById(id);
const threadList=$('threadList'), newThreadBtn=$('newThreadBtn'), logoutBtn=$('logoutBtn');
const fileInput=$('fileInput'), templateInput=$('templateInput'), referenceInput=$('referenceInput');
const chatForm=$('chatForm'), promptInput=$('promptInput'), sendBtn=$('sendBtn'), stopBtn=$('stopBtn');
const chat=$('chat'), preview=$('preview'), documentList=$('documentList'), activityLog=$('activityLog');
const previewMeta=$('previewMeta'), downloadLink=$('downloadLink'), renderBtn=$('renderBtn');
const statusDot=$('statusDot'), statusText=$('statusText');
const sourceSelect=$('sourceSelect'), templateSelect=$('templateSelect'), languageInput=$('languageInput'), glossaryInput=$('glossaryInput');
function emptyWorkflow() { return {mode:'template',sourceDocumentId:'',templateDocumentId:'',referenceDocumentIds:[],language:'Italian',glossary:''}; }
function workflow() {
  if (!state.draft) state.draft = state.thread?.workflow ? structuredClone(state.thread.workflow) : {...emptyWorkflow(), sourceDocumentId:state.thread?.currentDocumentId || ''};
  return state.draft;
}
function currentSource() { return state.thread?.documents.find(doc=>doc.id===workflow().sourceDocumentId); }
function showError(error) { addMessage('error', error.message || String(error)); }
function ready() { const w=workflow(); return Boolean(state.thread && w.sourceDocumentId && (w.mode==='translate' ? w.language.trim() : w.templateDocumentId)); }
function updateControls() {
  const w=workflow(); const translating=w.mode==='translate';
  $('templateMode').setAttribute('aria-pressed',String(!translating)); $('translateMode').setAttribute('aria-pressed',String(translating));
  $('templateSlot').hidden=translating; $('translationFields').hidden=!translating;
  promptInput.placeholder=translating ? 'For example: translate every slide into Italian. Use the French deck for context.' : 'For example: use the new template, keep all pictures and shorten long titles.';
  sendBtn.textContent=state.followup ? 'Send instructions' : translating ? 'Translate deck' : 'Change template';
  sendBtn.disabled=state.busy || !ready();
  const hint=!w.sourceDocumentId ? 'Upload a source deck to begin.' : !translating && !w.templateDocumentId ? 'Add the template you want to use.' : translating && !w.language.trim() ? 'Enter the language you want.' : 'Ready. Add instructions, or start with the settings above.';
  $('setupHint').textContent=hint;
  statusText.textContent=state.busy ? state.busyLabel : state.dirty && state.thread ? 'Settings will be saved when you start.' : hint;
  statusDot.classList.toggle('running',state.busy);
}
function setBusy(busy,label='Working…') {
  state.busy=busy; state.busyLabel=label;
  document.querySelectorAll('.setup input,.setup select,.setup textarea,.setup button,.thread-row,#newThreadBtn,#previewDocument,#previewVersion,#renderBtn').forEach(el=>el.disabled=busy || el.dataset.keepDisabled==='true');
  promptInput.disabled=busy; stopBtn.hidden=!state.stopStream;
  updateControls();
}
function updateStatus() { updateControls(); }
function markDirty() { state.dirty=true; updateControls(); }
for (const [id,mode] of [['templateMode','template'],['translateMode','translate']]) $(id).onclick=()=>{ workflow().mode=mode; state.followup=false; markDirty(); };
languageInput.oninput=()=>{workflow().language=languageInput.value;markDirty();};
glossaryInput.oninput=()=>{workflow().glossary=glossaryInput.value;markDirty();};
sourceSelect.onchange=()=>{
  const w=workflow();w.sourceDocumentId=sourceSelect.value;w.referenceDocumentIds=w.referenceDocumentIds.filter(id=>id!==w.sourceDocumentId);
  if(w.templateDocumentId===w.sourceDocumentId) w.templateDocumentId='';
  state.previewId=w.sourceDocumentId;state.previewVersion='latest';state.slide=0;markDirty();renderThread();ensurePreview();
};
templateSelect.onchange=()=>{const w=workflow();w.templateDocumentId=templateSelect.value;w.referenceDocumentIds=w.referenceDocumentIds.filter(id=>id!==w.templateDocumentId);markDirty();renderThread();};
$('glossaryFile').onchange=async event=>{
  const file=event.target.files[0];if(!file)return;
  try {
    if(!/\\.(txt|csv)$/i.test(file.name)||file.size>50000) throw Error('Use a TXT or CSV terms list up to 50 KB.');
    const text=await file.text();if(text.includes('\\u0000'))throw Error('Please save the terms list as UTF-8 text or CSV.');
    glossaryInput.value=text;workflow().glossary=text;markDirty();$('glossaryStatus').textContent='Loaded '+file.name+'. You can edit the terms above.';
  } catch(error) {showError(error);} finally {event.target.value='';}
};
logoutBtn.onclick=async()=>{await apiFetch('/api/auth/logout',{method:'POST'}).catch(()=>{});location.href=appUrl('/access');};
newThreadBtn.onclick=()=>{
  state.thread=null;state.draft=emptyWorkflow();state.previewId='';state.previewKey='';state.attemptedPreview='';state.followup=false;state.dirty=false;
  promptInput.value='';glossaryInput.value='';languageInput.value='Italian';$('glossaryStatus').textContent='';$('setupPanel').open=true;
  chat.innerHTML='<div class="chat-empty">Upload a deck, choose a task, then tell me what you need.</div>';renderThreads();renderThread();
};
for(const [input,role] of [[fileInput,'source'],[templateInput,'template'],[referenceInput,'reference']]) input.onchange=()=>uploadFiles(input,role);
async function uploadFiles(input,role) {
  const files=Array.from(input.files||[]);if(!files.length||state.busy)return;
  const oldIds=new Set(state.thread?.documents.map(doc=>doc.id)||[]);const w=structuredClone(workflow());
  setBusy(true,'Uploading '+(files.length===1?'file':'files')+'…');
  try {
    if(files.some(file=>! /\\.(pptx|pptm)$/i.test(file.name)))throw Error('Please choose a PowerPoint file (.pptx or .pptm).');
    const form=new FormData();for(const file of files)form.append('files',file);
    const url=state.thread ? '/api/threads/'+state.thread.id+'/upload' : '/api/upload';
    const data=await readApiJson(await apiFetch(url,{method:'POST',body:form}),'Upload');state.thread=data;
    const added=data.documents.filter(doc=>!oldIds.has(doc.id));
    if(role==='source') { w.sourceDocumentId=added[0].id;w.referenceDocumentIds.push(...added.slice(1).map(doc=>doc.id));state.previewId=w.sourceDocumentId;state.previewVersion='latest'; }
    if(role==='template') w.templateDocumentId=added[0].id;
    if(role==='reference') w.referenceDocumentIds.push(...added.map(doc=>doc.id));
    w.referenceDocumentIds=[...new Set(w.referenceDocumentIds)].filter(id=>id!==w.sourceDocumentId && id!==w.templateDocumentId);
    state.draft=w;await saveSettings();await loadThreads(data.id,false);renderThread();
    addMessage('trace',files.length+' file(s) uploaded');
  } catch(error) {showError(error);renderThread();} finally {input.value='';setBusy(false);}
  await ensurePreview();
}
async function saveSettings() {
  if(!state.thread)return;
  state.thread=await readApiJson(await apiFetch('/api/threads/'+state.thread.id+'/workflow',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(workflow())}),'Save task');
  state.dirty=false;
}
async function loadThreads(selectId,loadSelected=false) {
  const data=await readApiJson(await apiFetch('/api/threads'),'Previous work');state.threads=data.threads||[];
  if(selectId && loadSelected) state.thread=state.threads.find(t=>t.id===selectId)||state.thread;
  renderThreads();
}
function renderThreads() {
  threadList.innerHTML='';
  if(!state.threads.length) {threadList.textContent='Your jobs will appear here.';return;}
  for(const thread of state.threads) {
    const button=document.createElement('button');button.className='thread-row';button.disabled=state.busy;
    button.textContent=thread.title;button.onclick=()=>openThread(thread.id);threadList.append(button);
  }
}
async function openThread(id) {
  if(state.busy)return;setBusy(true,'Opening job…');
  try {
    state.thread=await readApiJson(await apiFetch('/api/threads/'+id),'Open job');state.draft=null;state.dirty=false;state.followup=false;
    state.previewId=workflow().sourceDocumentId||state.thread.currentDocumentId;state.previewVersion='latest';state.slide=0;
    languageInput.value=workflow().language;glossaryInput.value=workflow().glossary;promptInput.value='';
    chat.innerHTML='<div class="chat-empty">Job reopened. Your files and settings are saved. Add instructions to continue.</div>';
    $('recentWork').open=false;renderThread();
  } catch(error) {showError(error);} finally {setBusy(false);}await ensurePreview();
}
async function refreshThread() {
  if(!state.thread)return;state.thread=await readApiJson(await apiFetch('/api/threads/'+state.thread.id),'Refresh job');renderThread();
}
function fillSelect(select,docs,value,emptyLabel) {
  select.innerHTML='';if(emptyLabel){const o=new Option(emptyLabel,'');select.add(o);}
  for(const doc of docs)select.add(new Option(doc.originalName,doc.id));select.value=value;
}
function renderThread() {
  const docs=state.thread?.documents||[],w=workflow();
  sourceSelect.hidden=!docs.length;templateSelect.hidden=!docs.length;
  fillSelect(sourceSelect,docs,w.sourceDocumentId,'Choose a source deck');fillSelect(templateSelect,docs.filter(d=>d.id!==w.sourceDocumentId),w.templateDocumentId,'Choose a template');
  $('referenceList').textContent=w.referenceDocumentIds.map(id=>docs.find(d=>d.id===id)?.originalName).filter(Boolean).join(' · ');
  $('setupSummary').textContent=currentSource() ? 'Files & settings · '+currentSource().originalName : '1. Choose your task and files';
  $('filesPanel').hidden=!docs.length;documentList.innerHTML='';
  for(const doc of docs) {
    const row=document.createElement('div');row.className='doc-card';const name=document.createElement('span');
    const role=doc.id===w.sourceDocumentId?'Source':doc.id===w.templateDocumentId?'Template':w.referenceDocumentIds.includes(doc.id)?'Reference':'Unassigned';
    name.textContent=doc.originalName+' · '+role;row.append(name);
    if(role==='Unassigned'){const ref=document.createElement('button');ref.textContent='Use as reference';ref.onclick=()=>{w.referenceDocumentIds.push(doc.id);markDirty();renderThread();};row.append(ref);}
    const remove=document.createElement('button');remove.textContent='Remove';remove.dataset.keepDisabled=String(docs.length<2);remove.disabled=state.busy||docs.length<2;remove.onclick=()=>removeDocument(doc.id,doc.originalName);row.append(remove);documentList.append(row);
  }
  const source=currentSource();downloadLink.hidden=!source;
  if(source){downloadLink.href=appUrl(source.downloadUrl);downloadLink.textContent=source.versions.length>1?'Download result':'Download original';}
  $('resultHint').textContent=source?.versions.length>1 ? 'Latest saved result. Compare it with the original upload.' : 'Original files stay available.';
  $('previewControls').hidden=!docs.length;
  if(!docs.some(doc=>doc.id===state.previewId))state.previewId=w.sourceDocumentId||docs[0]?.id||'';
  fillSelect($('previewDocument'),docs,state.previewId);
  $('previewVersion').value=state.previewVersion;
  renderPreview();updateControls();
}
function previewSelection() {
  const doc=state.thread?.documents.find(doc=>doc.id===state.previewId);const versions=doc?.versions||[];
  const version=state.previewVersion==='original'?versions[0]:versions.find(v=>v.id===doc?.currentVersionId);
  return {doc,version,key:doc&&version ? state.thread.id+'/'+doc.id+'/'+version.id : ''};
}
function renderPreview() {
  const {doc,version,key}=previewSelection();
  if(state.previewKey!==key){state.slide=0;state.previewKey=key;}
  if(!doc){preview.innerHTML='<div class="empty"><h2>A fresh version of your slides.</h2><p>Start with the presentation you already have.</p><ol><li>Upload a source deck.</li><li>Add a template or choose a language.</li><li>Describe the changes, then review and download.</li></ol></div>';previewMeta.textContent='Your slides will appear here.';return;}
  const thumbs=version?.render?.thumbnails||[];
  $('previewVersion').options[0].textContent=doc.versions.length>1?'Latest result':'Current upload';
  previewMeta.textContent=doc.originalName;
  if(!thumbs.length){preview.innerHTML='<div class="empty">'+(doc.previewSupported?'Preparing your slide preview…':'This file has no slide preview. You can still download it.')+'</div>';return;}
  state.slide=Math.min(state.slide,thumbs.length-1);const thumb=thumbs[state.slide];preview.innerHTML='';
  const frame=document.createElement('div');frame.className='slide-frame';const img=document.createElement('img');img.src=appUrl(thumb.url);img.alt='Slide '+thumb.index+' of '+thumbs.length;frame.append(img);
  const nav=document.createElement('div');nav.className='slide-nav';
  const previous=document.createElement('button');previous.textContent='←';previous.setAttribute('aria-label','Previous slide');previous.disabled=state.slide===0;previous.onclick=()=>{state.slide--;renderPreview();};
  const next=document.createElement('button');next.textContent='→';next.setAttribute('aria-label','Next slide');next.disabled=state.slide===thumbs.length-1;next.onclick=()=>{state.slide++;renderPreview();};
  const count=document.createElement('span');count.className='subtle';count.textContent='Slide '+thumb.index+' of '+thumbs.length;
  nav.append(previous,count,next);preview.append(frame,nav);
}
async function ensurePreview(force=false) {
  const {doc,version,key}=previewSelection();if(state.busy||!doc?.previewSupported||!version)return;
  if(!force && (version.render?.thumbnails?.length||state.attemptedPreview===key))return;
  state.attemptedPreview=key;setBusy(true,'Preparing slide preview…');
  try {
    const url='/api/threads/'+state.thread.id+'/render?documentId='+encodeURIComponent(doc.id)+'&versionId='+encodeURIComponent(version.id);
    await readApiJson(await apiFetch(url,{method:'POST'}),'Preview');await refreshThread();
  } catch(error) {preview.innerHTML='<div class="empty">The preview could not be created. Use “Refresh preview” to try again. Your file is still available to download.</div>';showError(error);}
  finally {setBusy(false);}
}
$('previewDocument').onchange=()=>{state.previewId=$('previewDocument').value;state.slide=0;renderPreview();ensurePreview();};
$('previewVersion').onchange=()=>{state.previewVersion=$('previewVersion').value;state.slide=0;renderPreview();ensurePreview();};
renderBtn.onclick=()=>ensurePreview(true);
async function removeDocument(id,name) {
  if(state.busy||!confirm('Remove '+name+' from this job?'))return;setBusy(true,'Removing file…');
  try {
    state.thread=await readApiJson(await apiFetch('/api/threads/'+state.thread.id+'/documents/'+id,{method:'DELETE'}),'Remove file');
    const w=workflow();if(w.sourceDocumentId===id)w.sourceDocumentId='';if(w.templateDocumentId===id)w.templateDocumentId='';w.referenceDocumentIds=w.referenceDocumentIds.filter(ref=>ref!==id);
    await saveSettings();renderThread();
  } catch(error) {showError(error);}finally{setBusy(false);}await ensurePreview();
}
chatForm.onsubmit=async event=>{
  event.preventDefault();if(state.busy||!ready())return;
  const w=workflow(),request=promptInput.value.trim();if(state.followup&&!request){promptInput.focus();return;}
  const message=request||(w.mode==='translate'?'Translate the source deck into '+w.language+'.':'Adapt the source deck to the uploaded template.');
  setBusy(true,w.mode==='translate'?'Translating your slides…':'Updating your slides…');
  try {
    await saveSettings();const docs=state.thread.documents;
    const context={task:w.mode,source:docs.find(d=>d.id===w.sourceDocumentId)?.originalName,template:w.mode==='template'?docs.find(d=>d.id===w.templateDocumentId)?.originalName:undefined,language:w.mode==='translate'?w.language:undefined};
    addMessage('user',message);promptInput.value='';$('setupPanel').open=false;resetActivity('Starting');
    const body='Task settings: '+JSON.stringify(context)+'\\nUse get_thread_status to read the saved workflow, file IDs, references and preferred terms. Edit only the source deck.\\n\\n'+message;
    const data=await readApiJson(await apiFetch('/flue/agents/ooxml-editor/'+state.thread.id,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({kind:'user',body})}),'Request',agentErrorMessage);
    await streamAgentEvents(data);state.followup=true;
  } catch(error) {showError(error);if(!promptInput.value)promptInput.value=message;}
  finally {
    state.stopStream=null;await refreshThread().catch(showError);await loadThreads().catch(()=>{});
    state.previewId=w.sourceDocumentId;state.previewVersion='latest';renderThread();setBusy(false);
  }
  await ensurePreview();
};
stopBtn.onclick=()=>{if(state.stopStream)state.stopStream();};
function addMessage(role,text) {
  if(role==='trace')return logActivity(text);
  chat.querySelector('.chat-empty')?.remove();const node=document.createElement('div');node.className='message '+role;
  if(role==='assistant')node.innerHTML=renderMarkdown(text);else node.textContent=text;
  if(role==='error')node.setAttribute('role','alert');chat.append(node);chat.scrollTop=chat.scrollHeight;return node;
}
function resetActivity(text) {activityLog.innerHTML='';state.activityLines=[];logActivity(text);}
function logActivity(text,level='info') {
  const node=document.createElement('div');node.className='activity-line '+level;node.textContent=new Date().toLocaleTimeString()+' '+text;activityLog.append(node);
  while(activityLog.children.length>80)activityLog.firstChild.remove();activityLog.scrollTop=activityLog.scrollHeight;
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
	          url.searchParams.set('view', 'updates');
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
	              finish(new Error('Agent stream timed out before completion.'));
	            }
	          }, 5_000);
		          const finish = (error) => {
		            if (settled) return;
		            settled = true;
		            clearInterval(watchdog);
		            state.stopStream = null;
		            source.close();
		            if (error) reject(error); else resolve();
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
	              if (item?.type === 'submission-settled' && item.submissionId === admission.submissionId) {
                finish(item.outcome === 'completed' ? undefined : new Error('Agent submission ' + item.outcome + ' · ' + readableError(item.error)));
              }
	            }
	          });
	          source.addEventListener('control', (event) => {
	            lastEventAt = Date.now();
	            try {
              const control = JSON.parse(event.data);
              if (control.streamClosed) finish(new Error('Agent stream closed before completion.'));
            } catch {}
          });
	          source.onerror = () => {
	            if (!settled) {
	              source.close();
	              if (sawEvent || assistantText.trim()) {
	                addMessage('trace', 'event stream closed early · refreshing thread state');
	                finish(new Error('Agent stream disconnected before completion.'));
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
            case 'tool-input':
            case 'tool_start': addMessage('trace', 'tool started · ' + (event.toolName || 'unknown')); break;
            case 'tool':
            case 'tool_call': addMessage('trace', toolTraceText(event)); break;
            case 'message-delta':
              if (event.kind !== 'text') break;
              assistantText += event.delta || '';
              assistantNode.innerHTML = renderMarkdown(assistantText);
              chat.scrollTop = chat.scrollHeight;
              break;
            case 'tool-output': addMessage('trace', 'tool finished'); break;
            case 'tool-output-error': addMessage('trace', 'Tool attempt failed · ' + (event.errorText || 'unknown')); break;
            case 'submission-settled':
              if (event.submissionId !== admission.submissionId) break;
              if (event.outcome === 'completed') addMessage('trace', 'done');
              else addMessage('error', 'Agent submission ' + event.outcome + ' · ' + readableError(event.error));
              break;
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
          .replace(/\\[([^\\]]+)\\]\\((https?:\\/\\/[^\\s)]+|\\/(?!\\/)[^\\s)]+)\\)|(\\/api\\/[^\\s<)]+)/g,
            (match, label, url, bare) => '<a href="' + appUrl(url || bare) + '" rel="noopener noreferrer">' + (label || 'Download file') + '</a>');
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

loadThreads().catch(showError);
apiFetch('/api/auth/me').then(r=>readApiJson(r,'Sign-in')).then(data=>{if(data.csrfToken)state.csrfToken=data.csrfToken;}).catch(showError);
renderThread();
</script>
</body>
</html>`;
}
