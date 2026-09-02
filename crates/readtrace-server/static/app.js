const $ = (id) => document.getElementById(id);
const state = {
  files: [],
  units: [],
  batches: [],
  queue: [],
  selectedUnits: new Set(),
  expandedDirs: new Set(),
  fileView: 'essential',
  filter: 'all',
  pendingMerge: null,
  pendingUnitMerge: null,
  currentFile: null,
  processStatus: null,
  taskOutcomes: {},
  sessions: [],
  answerQuotes: new Map(),
  citationFiles: [],
  citationFilesLoaded: false,
  citationExpandedDirs: new Set(),
  citationSearchCache: new Map(),
  citationSearchSeq: 0,
  citationVaultId: null,
  filesRequestSeq: 0,
  pendingUpload: null,
};
const titles = { overview: '工作台', files: '文件浏览', import: '导入队列', process: '处理批次', backend: '后台', providers: '来源与 API', search: '检索', reader: '阅读与问答' };
const pretty = (value) => JSON.stringify(value, null, 2);
const api = async (url) => (await fetch(url)).json();
const post = async (url, body) => (await fetch(url, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(body) })).json();
const uploadFiles = async (files, options = {}) => {
  const form = new FormData();
  form.append('mode', options.mode || 'generic');
  form.append('order', options.order || 'filename');
  if (options.target) form.append('target', options.target);
  files.forEach((file) => form.append('files', file, file.webkitRelativePath || file.name));
  const response = await fetch('/api/import-upload', { method: 'POST', body: form });
  let result;
  try { result = await response.json(); } catch (_) { result = { ok: false, error: `上传失败（HTTP ${response.status}）` }; }
  if (!response.ok && result.ok !== false) result = { ok: false, error: `上传失败（HTTP ${response.status}）` };
  return result;
};
const show = (id, value) => { const node = $(id); if (node) node.textContent = typeof value === 'string' ? value : pretty(value); };
let toastTimer;
function toast(message, error = false) { const node = $('toast'); node.textContent = message; node.className = `toast visible${error ? ' error' : ''}`; clearTimeout(toastTimer); toastTimer = setTimeout(() => { node.className = 'toast'; }, 3200); }
function escapeHtml(value) { return String(value).replace(/[&<>"']/g, (character) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[character])); }
function formatBytes(bytes) { if (bytes < 1024) return `${bytes} B`; if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`; return `${(bytes / 1024 / 1024).toFixed(1)} MB`; }
function prepareSidebarUi() {
  const shell = document.querySelector('.app-shell');
  const toggle = $('sidebarToggle');
  if (!shell || !toggle) return;
  document.querySelectorAll('.nav-tab').forEach((tab) => {
    if (!tab.title) tab.title = tab.textContent.trim();
  });
  const apply = (collapsed) => {
    shell.classList.toggle('sidebar-collapsed', collapsed);
    toggle.textContent = collapsed ? '›' : '‹';
    toggle.title = collapsed ? '展开侧边栏' : '收窄侧边栏';
    toggle.setAttribute('aria-label', toggle.title);
    toggle.setAttribute('aria-pressed', String(collapsed));
  };
  let collapsed = false;
  try { collapsed = window.localStorage.getItem('readtrace.sidebarCollapsed') === 'true'; } catch (_) { /* private browsing */ }
  apply(collapsed);
  toggle.onclick = () => {
    collapsed = !collapsed;
    apply(collapsed);
    try { window.localStorage.setItem('readtrace.sidebarCollapsed', String(collapsed)); } catch (_) { /* private browsing */ }
  };
}
function prepareFilePickerUi() {
  const files = $('filePicker');
  const folder = $('folderPicker');
  const chooseFiles = $('chooseFiles');
  const chooseFolder = $('chooseFolder');
  const summary = $('uploadSelection');
  const copy = $('copy');
  if (!files || !folder || !chooseFiles || !chooseFolder || !summary) return;
  const setSelection = (list) => {
    state.pendingUpload = list.length ? [...list] : null;
    if (!state.pendingUpload) {
      summary.textContent = '也可以在下方输入本机路径';
      if (copy) copy.disabled = false;
      return;
    }
    const first = state.pendingUpload[0].webkitRelativePath || state.pendingUpload[0].name;
    const suffix = state.pendingUpload.length > 1 ? ` 等 ${state.pendingUpload.length} 个文件` : '';
    summary.textContent = `已选择 ${first}${suffix}`;
    if (copy) { copy.checked = true; copy.disabled = true; }
  };
  chooseFiles.onclick = () => files.click();
  chooseFolder.onclick = () => folder.click();
  files.onchange = () => { setSelection(files.files); files.value = ''; };
  folder.onchange = () => { setSelection(folder.files); folder.value = ''; };
  $('path')?.addEventListener('input', () => {
    if (!$('path').value.trim() || !state.pendingUpload) return;
    state.pendingUpload = null;
    summary.textContent = '也可以在下方输入本机路径';
    if (copy) copy.disabled = false;
  });
}
function prepareRepairPromptUi() {
  const panel = document.querySelector('#view-process .process-panel');
  const options = panel?.querySelector('.process-options');
  if (!panel || !options || $('repairPromptPanel')) return;
  const details = document.createElement('details');
  details.id = 'repairPromptPanel';
  details.className = 'prompt-panel';
  details.innerHTML = '<summary><span>编辑 LLM 修复提示词</span><span class="tag" id="repairPromptSource">项目默认</span></summary><div class="prompt-panel-body"><p class="prompt-hint">这里的提示词会保存到当前 Vault 的 <code>prompts/repair.md</code>，下一次 repair 自动使用。保留 <code>{mode}</code> 可适配不同输入类型。</p><textarea id="repairPromptEditor" spellcheck="false" placeholder="正在加载默认提示词…"></textarea><div class="prompt-actions"><span id="repairPromptStatus" class="muted"></span><button type="button" class="button secondary" id="repairPromptReload">重新加载</button><button type="button" class="text-button danger" id="repairPromptReset">恢复默认</button><button type="button" class="button primary" id="repairPromptSave">保存提示词</button></div></div>';
  options.after(details);
  const editor = $('repairPromptEditor');
  const source = $('repairPromptSource');
  const status = $('repairPromptStatus');
  const load = async () => {
    status.textContent = '加载中…';
    const result = await api('/api/prompts/repair');
    if (!result.ok) { status.textContent = result.error || '加载失败'; return; }
    editor.value = result.content || '';
    source.textContent = result.custom ? 'Vault 自定义' : '项目默认';
    status.textContent = result.hint || '已加载';
  };
  $('repairPromptReload').onclick = load;
  $('repairPromptSave').onclick = async () => {
    const content = editor.value;
    if (!content.trim()) return toast('提示词不能为空', true);
    status.textContent = '保存中…';
    const result = await post('/api/prompts/repair', { content });
    if (!result.ok) { status.textContent = result.error || '保存失败'; return toast(result.error || '保存失败', true); }
    source.textContent = 'Vault 自定义';
    status.textContent = '已保存，下一次 repair 自动使用';
    toast('修复提示词已保存');
  };
  $('repairPromptReset').onclick = async () => {
    if (!window.confirm('恢复项目默认提示词？当前 Vault 的自定义提示词会被删除。')) return;
    const result = await post('/api/prompts/repair', { reset: true });
    if (!result.ok) return toast(result.error || '恢复失败', true);
    editor.value = result.content || '';
    source.textContent = '项目默认';
    status.textContent = '已恢复项目默认提示词';
    toast('已恢复项目默认提示词');
  };
  load();
}
function prepareQueueCleanNameUi() {
  const button = $('queueAdd');
  if (!button) return;
  button.onclick = () => {
    const path = $('path').value.trim();
    const uploadFiles = state.pendingUpload ? [...state.pendingUpload] : null;
    if (!path && !uploadFiles?.length) return toast('请选择文件、文件夹，或输入本机路径', true);
    state.queue.push({
      path: path || `本地选择（${uploadFiles.length} 个文件）`,
      uploadFiles,
      mode: $('mode').value,
      copy: uploadFiles?.length ? true : $('copy').checked,
      ocr: $('queueOcr').value,
      provider: $('queueProvider').value,
      speed: $('queueSpeed').value,
      model: $('queueModel').value.trim(),
      cleanName: $('queueCleanName')?.value.trim() || '',
      merge: $('queueMerge').value,
    });
    $('path').value = '';
    state.pendingUpload = null;
    $('uploadSelection').textContent = '也可以在下方输入本机路径';
    $('copy').disabled = false;
    renderQueue();
  };
}
function go(view) { document.querySelectorAll('.nav-tab').forEach((tab) => tab.classList.toggle('active', tab.dataset.view === view)); document.querySelectorAll('.view').forEach((section) => section.classList.toggle('active', section.id === `view-${view}`)); $('viewTitle').textContent = titles[view] || view; if (view === 'files') loadFiles(); if (view === 'backend') { loadTasks(); loadActivity(); } if (view === 'reader') loadUsage(); }
function selectedBatch() { const batch = $('batch').value; if (!batch) { toast('先选择一个批次', true); return null; } return batch; }
function llmBody(provider) { const batch = selectedBatch(); if (!batch) return null; const body = { batch_id: batch, provider, speed: $('speed').value }; if ($('model').value.trim()) body.model = $('model').value.trim(); return body; }
function setBatchOptions(batches, preferred) { state.batches = batches || []; $('batchCount').textContent = state.batches.length; const select = $('batch'); const current = preferred || select.value; select.replaceChildren(); if (!state.batches.length) { select.add(new Option('暂无批次，请先导入', '')); return; } state.batches.forEach((batch) => select.add(new Option(`${batch.batch_id} · ${batch.status || 'imported'}`, batch.batch_id))); select.value = state.batches.some((batch) => batch.batch_id === current) ? current : state.batches[0].batch_id; updateBatchHint(); }
function updateBatchHint() { const batch = state.batches.find((item) => item.batch_id === $('batch').value); $('batchHint').textContent = batch ? `${batch.source_files?.length || 0} 个来源 · ${batch.mode || 'generic'}` : ''; }
async function loadWorkspace() { try { const [info, vaults] = await Promise.all([api('/api/vault'), api('/api/vaults')]); if (!info.ok || !vaults.ok) throw new Error(info.error || vaults.error || '无法读取 Workspace'); const workspace = vaults.workspace || info.workspace; $('workspaceName').textContent = workspace ? String(workspace).split(/[\\/]/).filter(Boolean).pop() : '单 Vault 模式'; $('root').textContent = info.root || vaults.selected || ''; $('currentVaultLabel').textContent = vaults.selected_vault?.name || '当前 Vault'; const list = $('vaultList'); list.replaceChildren(); (vaults.vaults || []).forEach((vault) => { const button = document.createElement('button'); button.className = `vault-item${vault.vault_id === vaults.selected_vault?.vault_id ? ' active' : ''}`; button.innerHTML = `<span class="vault-dot"></span><span>${escapeHtml(vault.name)}</span>`; button.onclick = async () => { const result = await post('/api/vaults/select', { name_or_id: vault.vault_id }); if (!result.ok) return toast(result.error, true); state.expandedDirs.clear(); state.currentFile = null; state.selectedUnits.clear(); state.pendingUnitMerge = null; state.filesRequestSeq += 1; toast(`已切换到 ${vault.name}`); await refresh(); }; list.append(button); }); } catch (error) { $('connectionStatus').textContent = '服务不可用'; toast(error.message, true); } }
async function loadBatches() { const result = await api('/api/batches'); if (result.ok) setBatchOptions(result.batches); }
function categoryCounts() { return { all: state.files.length, sources: state.files.filter((file) => file.path.startsWith('sources/')).length, generated: state.files.filter((file) => file.path.startsWith('generated/')).length, clean: state.files.filter((file) => file.path.startsWith('clean/')).length, audit: state.files.filter((file) => ['metadata.json', 'correction_log.json', 'events', 'runtime', 'raw', 'sessions'].some((name) => file.path === name || file.path.startsWith(`${name}/`))).length }; }
function fileMatches(file) { const search = $('fileSearch').value.trim().toLowerCase(); if (search && !`${file.name} ${file.path}`.toLowerCase().includes(search)) return false; if (state.filter === 'all') return true; if (state.filter === 'sources') return file.path.startsWith('sources/'); if (state.filter === 'generated') return file.path.startsWith('generated/'); if (state.filter === 'clean') return file.path.startsWith('clean/'); if (state.filter === 'audit') return ['metadata.json', 'correction_log.json', 'events', 'runtime', 'raw', 'sessions'].some((name) => file.path === name || file.path.startsWith(`${name}/`)); return true; }
function makeFileRow(file, depth) { const unit = state.units.find((item) => item.path === file.path); const row = document.createElement('div'); row.className = `file-row${state.currentFile?.path === file.path ? ' selected' : ''}`; row.style.paddingLeft = `${10 + depth * 16}px`; if (unit) { const check = document.createElement('input'); check.type = 'checkbox'; check.checked = state.selectedUnits.has(unit.unit_id); check.title = '选择此单元用于合并'; check.onclick = (event) => { event.stopPropagation(); if (check.checked) state.selectedUnits.add(unit.unit_id); else state.selectedUnits.delete(unit.unit_id); updateMergeDock(); }; row.append(check); } else { const spacer = document.createElement('span'); spacer.className = 'check-spacer'; row.append(spacer); } const icon = document.createElement('span'); icon.className = `file-type ${file.kind}`; icon.textContent = file.kind === 'image' ? '▧' : file.kind === 'pdf' ? '▥' : file.kind === 'text' ? '▤' : '•'; const info = document.createElement('span'); info.className = 'file-info'; info.innerHTML = `<strong>${escapeHtml(file.name)}</strong><small>${escapeHtml(file.path)} · ${formatBytes(file.size)}</small>`; const badge = document.createElement('span'); badge.className = 'file-badge'; badge.textContent = unit?.kind || file.category; row.append(icon, info, badge); row.onclick = () => previewFile(file); return row; }
function renderFiles() { const box = $('fileList'); if (!box) return; const visible = state.files.filter(fileMatches); const counts = categoryCounts(); document.querySelectorAll('#fileFilters .filter').forEach((button) => { const number = button.querySelector('em'); if (number) number.textContent = counts[button.dataset.filter] ?? ''; button.classList.toggle('active', button.dataset.filter === state.filter); }); $('selectionLabel').textContent = `${visible.length} 个文件`; box.replaceChildren(); if (!visible.length) { box.innerHTML = state.filter === 'clean' ? '<div class="empty-state">此 Vault 还没有 clean 文件。完成批次并生成 revision 后，结果会自动发布到 clean/。</div>' : '<div class="empty-state">没有匹配的文件</div>'; return; } const root = { dirs: new Map(), files: [] }; visible.forEach((file) => { const parts = file.path.split('/'); let node = root; parts.slice(0, -1).forEach((part) => { if (!node.dirs.has(part)) node.dirs.set(part, { dirs: new Map(), files: [] }); node = node.dirs.get(part); }); node.files.push(file); }); const draw = (node, prefix, depth) => { [...node.dirs.keys()].sort().forEach((name) => { const key = prefix ? `${prefix}/${name}` : name; const open = state.expandedDirs.has(key); const folder = document.createElement('button'); folder.className = 'folder-row'; folder.style.paddingLeft = `${10 + depth * 16}px`; folder.innerHTML = `<span class="folder-chevron">${open ? '⌄' : '›'}</span><span class="folder-icon">${open ? '▾' : '▸'}</span><strong>${escapeHtml(name)}</strong>`; folder.onclick = () => { if (open) state.expandedDirs.delete(key); else state.expandedDirs.add(key); renderFiles(); }; box.append(folder); if (open) draw(node.dirs.get(name), key, depth + 1); }); [...node.files].sort((a, b) => a.name.localeCompare(b.name, 'zh-CN')).forEach((file) => box.append(makeFileRow(file, depth))); }; draw(root, '', 0); updateMergeDock(); }
function expandAllFolders(files = state.files) { files.forEach((file) => { const parts = file.path.split('/'); for (let index = 1; index < parts.length; index += 1) state.expandedDirs.add(parts.slice(0, index).join('/')); }); renderFiles(); }
function revealVisibleFolders() { state.expandedDirs.clear(); expandAllFolders(state.files.filter(fileMatches)); }
async function loadFiles() { const requestSeq = ++state.filesRequestSeq; const view = state.fileView; const result = await api(`/api/files?view=${view}`); if (requestSeq !== state.filesRequestSeq || view !== state.fileView) return; if (!result.ok) return toast(result.error, true); state.files = result.files || []; $('fileCount').textContent = state.files.length; renderFiles(); const recent = $('recentFiles'); recent.replaceChildren(); state.files.slice(-5).reverse().forEach((file) => { const row = document.createElement('button'); row.className = 'recent-file'; row.innerHTML = `<span class="file-type ${file.kind}">${file.kind === 'image' ? '▧' : '▤'}</span><span><strong>${escapeHtml(file.name)}</strong><small>${escapeHtml(file.path)}</small></span>`; row.onclick = () => { go('files'); previewFile(file); }; recent.append(row); }); recent.className = state.files.length ? 'recent-files' : 'recent-files empty-state'; if (!state.expandedDirs.size && state.files.length) expandAllFolders(state.files.filter(fileMatches)); }
async function loadUnits() { const result = await api('/api/sources'); if (!result.ok) return; state.units = result.units || []; $('unitCount').textContent = state.units.length; renderFiles(); }
async function previewFile(file) { state.currentFile = file; renderFiles(); $('previewTitle').textContent = file.name; const box = $('previewContent'); if (file.kind === 'image') { box.innerHTML = `<img class="image-preview" src="${file.raw_url}" alt="${escapeHtml(file.name)}"><div class="preview-meta">${escapeHtml(file.path)} · ${formatBytes(file.size)}</div>`; return; } if (file.kind === 'pdf') { box.innerHTML = `<iframe class="pdf-preview" src="${file.raw_url}" title="${escapeHtml(file.name)}"></iframe>`; return; } const result = await api(`/api/file?path=${encodeURIComponent(file.path)}`); if (!result.ok) return box.textContent = result.error; box.innerHTML = `<pre class="code-preview">${escapeHtml(result.content || '该文件没有可显示的文本内容。')}</pre>${result.truncated ? '<small class="muted">内容过长，仅显示前 120,000 个字符。</small>' : ''}`; }
function updateMergeDock() { $('mergeSelectionCount').textContent = state.selectedUnits.size; $('mergeConfirmUnits').disabled = !state.pendingUnitMerge; }
function setProcessStatus(label, tone = 'idle', detail = '', taskId = null) { state.processStatus = { label, tone, detail, taskId }; const node = $('processStatus'); if (!node) return; node.className = `process-status ${tone}`; node.innerHTML = `<span class="process-status-dot"></span><span><strong>${escapeHtml(label)}</strong>${detail ? `<small>${escapeHtml(detail)}</small>` : ''}</span>`; }
function renderProcessStatus(tasks) { const batch = $('batch')?.value; const relevant = (tasks || []).filter((task) => !batch || task.batch_id === batch).sort((a, b) => new Date(a.updated_at || 0) - new Date(b.updated_at || 0)); const task = relevant.at(-1); const current = state.processStatus; if (!task) { const meta = state.batches.find((item) => item.batch_id === batch); const persisted = { imported: ['等待处理', 'idle'], ocr_complete: ['OCR 已完成', 'completed'], repair_complete: ['LLM 修复 已完成', 'completed'], repair_partial: ['LLM 修复 完成但有错误', 'warning'], built: ['revision 已生成', 'completed'], applied: ['修订已应用', 'completed'], corrections_proposed: ['等待人工确认', 'warning'], cancelled: ['已取消', 'cancelled'] }[meta?.status]; if (persisted) setProcessStatus(persisted[0], persisted[1], '批次状态已从 Vault 恢复'); else if (!current || current.tone === 'running') setProcessStatus(batch ? '等待处理' : '选择批次后开始处理', 'idle'); return; } const label = task.kind === 'ocr' ? 'OCR' : task.kind === 'repair' ? 'LLM 修复' : task.kind; const detail = task.status === 'running' ? `${task.current || 0}/${task.total || '—'} · ${task.message || ''}` : task.error || (task.result ? pretty(task.result) : '可以继续下一步'); const finish = task.status === 'completed' ? ['已完成', 'completed'] : task.status === 'completed_with_errors' ? ['完成但有错误', 'warning'] : task.status === 'cancelled' ? ['已取消', 'cancelled'] : ['失败', 'failed']; if (task.status === 'running') setProcessStatus(`${label} 处理中…`, 'running', detail, task.task_id); else if (current?.tone === 'running' && current.taskId === task.task_id) setProcessStatus(`${label} ${finish[0]}`, finish[1], detail, task.task_id); else if (!current || current.taskId === task.task_id) setProcessStatus(`${label} ${finish[0]}`, finish[1], detail, task.task_id); }
async function loadTasks() { const result = await api('/api/tasks'); const tasks = result.tasks || []; $('runningCount').textContent = tasks.filter((task) => task.status === 'running').length; renderProcessStatus(tasks); const box = $('tasks'); if (!box) return; box.replaceChildren(); if (!tasks.length) { box.innerHTML = '<div class="empty-state">当前服务没有正在追踪的任务；历史完成态请看上方命令终端。</div>'; return; } const labels = { running: '处理中', completed: '已完成', completed_with_errors: '完成但有错误', failed: '失败', cancelled: '已取消' }; tasks.forEach((task) => { const row = document.createElement('article'); row.className = `task-card ${task.status}`; const percent = task.total ? Math.min(100, Math.round((task.current / task.total) * 100)) : 0; row.innerHTML = `<div class="task-card-head"><span class="task-kind">${escapeHtml(task.kind)}</span><span class="task-status">${escapeHtml(labels[task.status] || task.status)}</span></div><strong>${escapeHtml(task.batch_id || '—')}</strong><p>${escapeHtml(task.message || task.error || '等待处理…')}</p><div class="progress"><i style="width:${percent}%"></i></div><small>${task.current}/${task.total || '—'} · ${new Date(task.updated_at).toLocaleString()}</small>`; if (task.status === 'running') { const cancel = document.createElement('button'); cancel.className = 'text-button danger'; cancel.textContent = '取消任务'; cancel.onclick = async () => { await post(`/api/tasks/${encodeURIComponent(task.task_id)}/cancel`, {}); loadTasks(); }; row.append(cancel); } box.append(row); }); }
function progressDone(event) { if (event.type !== 'progress') return false; const message = String(event.message || ''); return (Number.isFinite(event.total) && event.total > 0 && event.current >= event.total) || /\b(ready|complete|completed|done|finished|success)\b|就绪|完成|成功/i.test(message); }
function eventPresentation(event) { const type = event.type || 'event'; if (type === 'task_started') return { label: '任务开始', detail: event.task_id || '—', tone: 'neutral' }; if (type === 'task_completed') return { label: '任务完成', detail: event.task_id || '—', tone: 'completed' }; if (type === 'task_cancelled') return { label: '任务取消', detail: event.reason || '—', tone: 'cancelled' }; if (type === 'error') return { label: '失败', detail: event.message || '—', tone: 'failed' }; if (type === 'warning') return { label: '警告', detail: event.message || '—', tone: 'warning' }; if (type === 'progress') return { label: event.stage || '处理中', detail: `${event.current || 0}/${event.total || 0} · ${event.message || ''}`, tone: progressDone(event) ? 'completed' : 'running' }; if (type === 'tool_requested') return { label: '调用工具', detail: event.tool_name || '—', tone: 'running' }; if (type === 'tool_completed') return { label: event.success ? '工具完成' : '工具失败', detail: `${event.tool_name || '—'} · ${event.duration_ms || 0} ms`, tone: event.success ? 'completed' : 'failed' }; return { label: type.replaceAll('_', ' '), detail: event.correction_id || event.operation || '—', tone: 'neutral' }; }
function terminalLine(event, sequence, taskOutcomes = state.taskOutcomes) { const presentation = eventPresentation(event); const time = event.created_at ? new Date(event.created_at).toLocaleTimeString([], { hour12: false }) : `#${String(sequence).padStart(4, '0')}`; let command = presentation.detail; if (event.type === 'task_started') command = `readtrace task start ${event.task_id || ''}`; else if (event.type === 'task_completed') command = `readtrace task complete ${event.task_id || ''}`; else if (event.type === 'task_cancelled') command = `readtrace task cancel ${event.task_id || ''}`; else if (event.type === 'error') command = `readtrace task failed ${event.message || ''}`; else if (event.type === 'warning') command = `readtrace task warning ${event.message || ''}`; else if (event.type === 'progress') command = `${event.stage || 'task'}  ${event.message || ''} [${event.current || 0}/${event.total || 0}]`; else if (event.type === 'tool_requested') command = `tool ${event.tool_name || ''}`; else if (event.type === 'tool_completed') command = `tool ${event.tool_name || ''} ${event.success ? 'completed' : 'failed'} (${event.duration_ms || 0} ms)`; const historical = event.type === 'task_started' ? (taskOutcomes[event.task_id] || state.taskOutcomes[event.task_id]) : null; const status = historical === 'completed' ? 'DONE' : historical === 'failed' ? 'ERROR' : historical === 'cancelled' ? 'CANCELLED' : event.type === 'task_completed' || (event.type === 'progress' && progressDone(event)) ? 'DONE' : event.type === 'error' ? 'ERROR' : event.type === 'warning' ? 'WARN' : event.type === 'task_cancelled' ? 'CANCELLED' : event.type === 'tool_completed' ? (event.success ? 'DONE' : 'ERROR') : event.type === 'task_started' || event.type === 'progress' || event.type === 'tool_requested' ? 'RUNNING' : 'INFO'; const tone = historical === 'completed' ? 'completed' : historical === 'failed' ? 'failed' : historical === 'cancelled' ? 'cancelled' : presentation.tone; return { time, command: command.trim(), tone, status }; }
async function loadActivity() { const result = await api('/api/activity'); if (!result.ok) return; const events = result.events || []; const tasks = result.tasks || []; const taskOutcomes = {}; events.forEach((event) => { if (event.task_id && event.type === 'task_completed') taskOutcomes[event.task_id] = 'completed'; if (event.task_id && event.type === 'task_cancelled') taskOutcomes[event.task_id] = 'cancelled'; if (event.task_id && event.type === 'error') taskOutcomes[event.task_id] = 'failed'; }); const usage = result.usage || {}; $('commandCount').textContent = events.length; $('completedCount').textContent = Math.max(events.filter((event) => event.type === 'task_completed').length, tasks.filter((task) => task.status === 'completed').length); $('failedCount').textContent = Math.max(events.filter((event) => event.type === 'error' || event.type === 'task_cancelled').length, tasks.filter((task) => task.status === 'failed' || task.status === 'cancelled').length); $('tokenCount').textContent = usage.total_tokens == null ? '—' : Number(usage.total_tokens).toLocaleString(); $('backendCostUsd').textContent = typeof usage.cost_usd === 'number' ? `$${usage.cost_usd.toFixed(6)}` : '—'; $('backendCostCny').textContent = typeof usage.cost_cny === 'number' ? `¥${usage.cost_cny.toFixed(4)}` : '—'; $('usageVersion').textContent = usage.unknown_cost_calls ? `${usage.unknown_cost_calls} 条未返回 usage` : 'priced'; if ($('backendUsageOut')) show('backendUsageOut', usage); const box = $('activity'); if (!box) return; box.replaceChildren(); if (!events.length) { box.innerHTML = '<div class="empty-state">还没有命令记录</div>'; return; } const compact = []; events.forEach((event, index) => { const line = terminalLine(event, index + 1, taskOutcomes); const previous = compact.at(-1); if (event.type === 'progress' && previous?.event?.type === 'progress' && previous.event.stage === event.stage && previous.event.task_id === event.task_id) { compact[compact.length - 1] = { event, line }; } else compact.push({ event, line }); }); compact.slice().reverse().forEach(({ line }) => { const row = document.createElement('div'); row.className = `activity-row ${line.tone}`; row.innerHTML = `<span class="terminal-time">${escapeHtml(line.time)}</span><span class="terminal-prompt">›</span><span class="activity-copy"><strong>${escapeHtml(line.command)}</strong></span><span class="activity-type">${escapeHtml(line.status)}</span>`; box.append(row); }); }
async function loadUsage() { const result = await api('/api/usage'); const summary = result.summary || result; $('costCount').textContent = typeof summary.cost_usd === 'number' ? summary.cost_usd.toFixed(6) : '—'; if ($('usageOut')) show('usageOut', summary); }
async function refresh() { await Promise.all([loadWorkspace(), loadBatches(), loadFiles(), loadUnits(), loadUsage(), loadTasks()]); if ($('view-backend').classList.contains('active')) await loadActivity(); }
async function waitTask(taskId) { for (let attempt = 0; attempt < 180; attempt += 1) { const result = await api(`/api/tasks/${encodeURIComponent(taskId)}`); if (result.task && result.task.status !== 'running') return result.task; await new Promise((resolve) => setTimeout(resolve, 1000)); } throw new Error('任务等待超时'); }
async function runQueuedPipeline(batchId, item) { const ocr = await post('/api/ocr', { batch_id: batchId, provider: item.ocr }); if (!ocr.ok) throw new Error(ocr.error); const ocrTask = await waitTask(ocr.task_id); if (ocrTask.status !== 'completed') throw new Error(ocrTask.error || 'OCR 失败'); const normalized = await post('/api/normalize', { batch_id: batchId }); if (!normalized.ok) throw new Error(normalized.error); const repair = await post('/api/repair', { batch_id: batchId, provider: item.provider, speed: item.speed, ...(item.model ? { model: item.model } : {}) }); if (!repair.ok) throw new Error(repair.error); const repairTask = await waitTask(repair.task_id); if (repairTask.status !== 'completed') throw new Error(repairTask.error || 'LLM 修复失败'); const merged = await post('/api/merge', { batch_id: batchId, confirm: true }); if (!merged.ok) throw new Error(merged.error); return merged; }
function renderQueue() { const box = $('importQueue'); $('queueCount').textContent = state.queue.length; $('queueRun').disabled = !state.queue.length; box.replaceChildren(); if (!state.queue.length) { box.className = 'queue-list empty-state'; box.textContent = '还没有待导入素材'; return; } box.className = 'queue-list'; state.queue.forEach((item, index) => { const row = document.createElement('div'); row.className = 'queue-item'; const clean = item.cleanName ? ` · clean/${item.cleanName}/document.md` : ''; const title = item.uploadFiles?.length ? `本地选择 · ${item.uploadFiles.length} 个文件` : item.path; const detail = item.uploadFiles?.length ? item.uploadFiles.map((file) => file.webkitRelativePath || file.name).slice(0, 2).join('、') : item.path; row.innerHTML = `<span class="queue-index">${String(index + 1).padStart(2, '0')}</span><span><strong>${escapeHtml(title)}</strong><small>${escapeHtml(detail)} · ${item.mode} · ${item.copy ? '复制原素材' : '外部引用'}${escapeHtml(clean)}</small></span>`; const remove = document.createElement('button'); remove.className = 'icon-button'; remove.textContent = '×'; remove.onclick = () => { state.queue.splice(index, 1); renderQueue(); }; row.append(remove); box.append(row); }); }
async function runQueue() { if (!state.queue.length) return; const items = [...state.queue]; state.queue = []; renderQueue(); const results = []; let previewBatch = null; for (const item of items) { try { const result = await post('/api/import', { path: item.path, mode: item.mode, no_copy: !item.copy }); if (!result.ok) throw new Error(result.error); const batchId = result.batch.batch_id; results.push(batchId); if (item.merge === 'auto') await runQueuedPipeline(batchId, item); if (item.merge === 'preview' && !previewBatch) previewBatch = batchId; toast(`已导入 ${batchId}`); } catch (error) { results.push(`失败：${error.message}`); toast(error.message, true); } } show('importOut', { batches: results }); await refresh(); const first = results.find((value) => !value.startsWith('失败')); if (previewBatch || ($('afterImport').value === 'process' && first)) { $('batch').value = previewBatch || first; go('process'); } else go('files'); }
async function processOcr() { const batch = selectedBatch(); if (!batch) return; setProcessStatus('OCR 启动中…', 'running'); const result = await post('/api/ocr', { batch_id: batch, provider: $('ocrProvider').value }); show('processOut', result); if (result.ok) { setProcessStatus('OCR 处理中…', 'running', '', result.task_id); toast('OCR 已开始'); loadTasks(); } else { setProcessStatus('OCR 启动失败', 'failed', result.error || ''); toast(result.error, true); } }
async function processNormalize() { const batch = selectedBatch(); if (!batch) return; setProcessStatus('规范化处理中…', 'running'); const result = await post('/api/normalize', { batch_id: batch, refresh: true }); show('processOut', result); if (result.ok) { const changed = (result.report?.pages || []).reduce((sum, page) => sum + (page.changes?.length || 0), 0); setProcessStatus('规范化已完成', 'completed', `${changed} 处确定性修改`); toast('确定性清洗完成'); } else { setProcessStatus('规范化失败', 'failed', result.error || ''); toast(result.error, true); } }
async function processRepair() { const body = llmBody($('provider').value); if (!body) return; body.refresh = true; setProcessStatus('LLM 修复启动中…', 'running'); const result = await post('/api/repair', body); show('processOut', result); if (result.ok) { setProcessStatus('LLM 修复处理中…', 'running', '', result.task_id); toast('LLM 修复已开始'); loadTasks(); } else { setProcessStatus('LLM 修复启动失败', 'failed', result.error || ''); toast(result.error, true); } }
async function processMerge(confirm) { const batch = selectedBatch(); if (!batch) return; const allowUnrepaired = $('allowUnrepaired')?.checked || false; const cleanName = $('cleanName')?.value.trim() || undefined; setProcessStatus(confirm ? '正在生成 revision…' : '正在生成合并预览…', 'running'); const result = await post('/api/merge', { batch_id: batch, confirm, allow_unrepaired: allowUnrepaired, clean_name: cleanName }); show('processOut', result); if (!result.ok) { setProcessStatus('合并失败', 'failed', result.error || ''); return toast(result.error, true); } state.pendingMerge = result.plan; $('confirmMerge').disabled = !result.confirmation_required; if (result.confirmation_required) { const warning = result.warning || (allowUnrepaired ? '本次预览允许使用未修复 OCR' : '请检查合并预览并确认'); setProcessStatus('等待合并确认', warning.includes('未修复') ? 'warning' : 'idle', warning); toast(warning); } else { const warning = result.warning || ''; setProcessStatus(warning ? 'revision 已生成（含未修复 OCR）' : 'revision 已生成', warning ? 'warning' : 'completed', result.clean_path || result.artifact?.path || warning); toast(result.clean_path ? `已发布到 ${result.clean_path}` : (warning || '已生成 revision')); await refresh(); } }
async function viewArtifact() { const batch = selectedBatch(); if (!batch) return; const result = await api(`/api/artifact?batch_id=${encodeURIComponent(batch)}`); show('artifactOut', result); if (!result.ok) toast(result.error, true); }
async function mergeUnits(confirm) { if (!confirm && state.selectedUnits.size < 1) return toast('至少选择一个 source 或 clean 单元', true); const allowUnrepaired = $('allowUnrepaired')?.checked || $('allowUnrepairedUnits')?.checked || false; const cleanName = $('cleanName')?.value.trim() || undefined; const body = confirm ? { confirm: true, merge_id: state.pendingUnitMerge?.merge_id, allow_unrepaired: allowUnrepaired, clean_name: cleanName } : { confirm: false, units: [...state.selectedUnits], allow_unrepaired: allowUnrepaired, clean_name: cleanName }; if (confirm && !body.merge_id) return toast('请先预览合并计划', true); const result = await post('/api/merge-units', body); if (!result.ok) return toast(result.error, true); state.pendingUnitMerge = result.plan; $('mergeConfirmUnits').disabled = !result.confirmation_required; if (result.confirmation_required) { $('previewTitle').textContent = '合并预览'; $('previewContent').innerHTML = `<pre class="code-preview">${escapeHtml(pretty(result.plan))}</pre>`; } else { const warning = result.warning || ''; toast(result.clean_path ? `已发布到 ${result.clean_path}` : (warning || '跨 batch revision 已生成')); await loadFiles(); } }
async function search() { const query = $('query').value.trim(); if (!query) return; const result = await api(`/api/search?q=${encodeURIComponent(query)}`); const box = $('searchOut'); if (!result.ok) return box.textContent = result.error; const hits = result.hits || []; box.className = hits.length ? 'search-results' : 'search-results empty-state'; box.replaceChildren(); hits.forEach((hit) => { const row = document.createElement('div'); row.className = 'search-hit'; row.innerHTML = `<strong>${escapeHtml(hit.path || hit.source_ref || '命中')}</strong><p>${escapeHtml(hit.snippet || hit.text || JSON.stringify(hit))}</p>`; box.append(row); }); if (!hits.length) box.textContent = '没有找到匹配内容'; }
async function answer() { const question = $('question').value.trim(); if (!question) return toast('先写下问题', true); const refs = [...state.selectedUnits].flatMap((id) => { const unit = state.units.find((item) => item.unit_id === id); return unit?.kind === 'clean' ? unit.source_refs || [] : []; }); const result = await post('/api/answer', { query: question, provider: $('answerProvider').value, speed: $('answerSpeed').value, source_refs: refs }); const box = $('answerOut'); if (!result.ok) return box.textContent = result.error; box.innerHTML = `<p>${escapeHtml(result.answer || '')}</p><small>引用 ${result.source_refs?.length || 0} 条 · ${escapeHtml(result.session_id || '')}</small>`; await loadUsage(); }
async function deleteSelected() { const units = [...state.selectedUnits]; if (!units.length) return toast('先选择要删除的 source / clean 单元', true); if (!window.confirm(`确定删除 ${units.length} 个单元及其派生文件吗？此操作不可恢复。`)) return; for (const unit of units) { const result = await post('/api/delete-unit', { unit, confirm: true }); if (!result.ok) return toast(result.error, true); } state.selectedUnits.clear(); state.pendingUnitMerge = null; toast('已删除选中的单元'); await refresh(); }
function wire() { document.querySelectorAll('.nav-tab').forEach((tab) => { tab.onclick = () => go(tab.dataset.view); }); document.querySelectorAll('[data-go]').forEach((button) => { button.onclick = () => go(button.dataset.go); }); $('refresh').onclick = refresh; $('openImport').onclick = () => go('import'); $('filesImport').onclick = () => go('import'); $('newWorkspace').onclick = () => $('workspaceDialog').showModal(); $('newVault').onclick = () => $('vaultDialog').showModal(); document.querySelectorAll('dialog button[value="cancel"]').forEach((button) => { button.onclick = (event) => { event.preventDefault(); button.closest('dialog').close(); }; }); $('workspaceForm').onsubmit = async (event) => { if (event.submitter?.value === 'cancel') return; event.preventDefault(); const result = await post('/api/workspace/init', { path: $('workspacePath').value, vault_name: $('workspaceVault').value }); if (!result.ok) return toast(result.error, true); $('workspaceDialog').close(); toast('Workspace 已创建并切换'); refresh(); }; $('vaultForm').onsubmit = async (event) => { if (event.submitter?.value === 'cancel') return; event.preventDefault(); const result = await post('/api/vaults/create', { name: $('vaultName').value, select: $('vaultSelect').checked }); if (!result.ok) return toast(result.error, true); $('vaultDialog').close(); toast('Vault 已创建'); refresh(); }; $('queueAdd').onclick = () => { const path = $('path').value.trim(); if (!path) return toast('请输入文件或文件夹路径', true); state.queue.push({ path, mode: $('mode').value, copy: $('copy').checked, ocr: $('queueOcr').value, provider: $('queueProvider').value, speed: $('queueSpeed').value, model: $('queueModel').value.trim(), merge: $('queueMerge').value }); $('path').value = ''; renderQueue(); }; $('queueClear').onclick = () => { state.queue = []; renderQueue(); }; $('queueRun').onclick = runQueue; document.querySelectorAll('#fileFilters .filter').forEach((button) => { button.onclick = () => { state.filter = button.dataset.filter; if (state.filter === 'audit' && state.fileView !== 'all') { state.fileView = 'all'; state.expandedDirs.clear(); $('essentialFiles').classList.remove('active'); $('allFiles').classList.add('active'); loadFiles(); } else revealVisibleFolders(); }; }); $('essentialFiles').onclick = () => { state.fileView = 'essential'; state.expandedDirs.clear(); $('essentialFiles').classList.add('active'); $('allFiles').classList.remove('active'); loadFiles(); }; $('allFiles').onclick = () => { state.fileView = 'all'; state.expandedDirs.clear(); $('allFiles').classList.add('active'); $('essentialFiles').classList.remove('active'); loadFiles(); }; $('fileSearch').oninput = renderFiles; $('selectAllFiles').onclick = () => { state.files.filter(fileMatches).forEach((file) => { const unit = state.units.find((item) => item.path === file.path); if (unit) state.selectedUnits.add(unit.unit_id); }); renderFiles(); }; $('deleteSelected').onclick = deleteSelected; $('mergePreviewUnits').onclick = () => mergeUnits(false); $('mergeConfirmUnits').onclick = () => mergeUnits(true); $('closePreview').onclick = () => { state.currentFile = null; $('previewTitle').textContent = '选择一个文件'; $('previewContent').innerHTML = '<div class="preview-placeholder"><span>◌</span><p>点击文件查看内容</p><small>从左侧选择图片、PDF 或文本文件</small></div>'; renderFiles(); }; $('batch').onchange = () => { state.pendingMerge = null; $('confirmMerge').disabled = true; updateBatchHint(); setProcessStatus($('batch').value ? '等待处理' : '选择批次后开始处理', 'idle'); }; $('processRefresh').onclick = loadBatches; $('ocr').onclick = processOcr; $('normalize').onclick = processNormalize; $('repair').onclick = processRepair; $('merge').onclick = () => processMerge(false); $('confirmMerge').onclick = () => processMerge(true); $('build').onclick = async () => { const batch = selectedBatch(); if (!batch) return; const meta = state.batches.find((item) => item.batch_id === batch); if ((meta?.source_files?.length || 0) > 1) return processMerge(false); const allowUnrepaired = $('allowUnrepaired')?.checked || false; const result = await post('/api/build', { batch_id: batch, allow_unrepaired: allowUnrepaired }); show('processOut', result); if (result.ok) { const warning = result.warning || (allowUnrepaired ? '本次使用了未修复 OCR' : ''); setProcessStatus(warning ? 'revision 已生成（含未修复 OCR）' : 'revision 已生成', warning ? 'warning' : 'completed', result.artifact?.path || warning); toast(warning || 'revision 已生成'); await refresh(); } else { setProcessStatus('生成失败', 'failed', result.error || ''); toast(result.error, true); } }; $('viewArtifact').onclick = viewArtifact; $('tasksRefresh').onclick = () => { loadTasks(); loadActivity(); }; $('activityRefresh').onclick = loadActivity; $('search').onclick = search; $('query').onkeydown = (event) => { if (event.key === 'Enter') search(); }; $('answer').onclick = answer; $('usageRefresh').onclick = loadUsage; }
function ensureUnitsOption() { const dock = $('mergeDock'); if (!dock || $('allowUnrepairedUnits')) return; const label = document.createElement('label'); label.className = 'merge-warning-option'; label.innerHTML = '<input id="allowUnrepairedUnits" type="checkbox"><span><strong>允许未修复 OCR</strong><small>仅视觉页修复失败时使用</small></span>'; dock.insertBefore(label, dock.lastElementChild); }
function wireCleanPublish() { const button = $('build'); if (!button) return; button.onclick = async () => { const batch = selectedBatch(); if (!batch) return; const meta = state.batches.find((item) => item.batch_id === batch); if ((meta?.source_files?.length || 0) > 1) return processMerge(false); const allowUnrepaired = $('allowUnrepaired')?.checked || false; const cleanName = $('cleanName')?.value.trim() || undefined; const result = await post('/api/build', { batch_id: batch, allow_unrepaired: allowUnrepaired, clean_name: cleanName }); show('processOut', result); if (result.ok) { const warning = result.warning || (allowUnrepaired ? '本次使用了未修复 OCR' : ''); setProcessStatus(warning ? 'revision 已生成（含未修复 OCR）' : 'revision 已生成', warning ? 'warning' : 'completed', result.clean_path || result.artifact?.path || warning); toast(result.clean_path ? `已发布到 ${result.clean_path}` : (warning || 'revision 已生成')); await refresh(); } else { setProcessStatus('生成失败', 'failed', result.error || ''); toast(result.error, true); } }; }
const originalTerminalLine = terminalLine;
terminalLine = function resolvedTerminalLine(event, sequence, taskOutcomes = state.taskOutcomes) { const line = originalTerminalLine(event, sequence, taskOutcomes); if (event.type === 'task_started' && line.status === 'RUNNING') line.status = 'INFO'; if (event.type === 'progress' && line.status === 'RUNNING') { line.status = 'INFO'; line.tone = 'neutral'; } return line; };
async function syncTaskOutcomes() { const result = await api('/api/tasks'); (result.tasks || []).forEach((task) => { state.taskOutcomes[task.task_id] = task.status === 'running' ? 'running' : task.status === 'cancelled' ? 'cancelled' : task.status === 'failed' || task.status === 'completed_with_errors' ? 'failed' : 'completed'; }); }
ensureUnitsOption(); wire(); wireCleanPublish(); renderQueue(); refresh(); syncTaskOutcomes(); setInterval(loadTasks, 2500); setInterval(loadActivity, 3000); setInterval(syncTaskOutcomes, 2500);

// Reader enhancements: keep the existing compact page markup, but turn its
// result panel into a small conversation surface.  These overrides are kept
// here so an older cached HTML shell remains compatible with the new API.
function prepareReaderUi() {
  state.answerSessionId = null;
  state.chatMessages = [];
  state.answerRefs = new Set();
  state.answerQuotes = new Map();
  const intro = document.querySelector('#view-reader .view-intro p');
  if (intro) intro.textContent = '搜索只使用 clean 本地索引；问答可以引用 clean 文件。';
  createSessionPanel();
  const output = $('answerOut');
  if (output) {
    output.className = 'chat-history';
    output.setAttribute('aria-live', 'polite');
  }
  const speed = $('answerSpeed');
  if (speed) {
    const label = speed.closest('label');
    if (label?.firstChild?.nodeType === Node.TEXT_NODE) label.firstChild.textContent = '推理挡位';
    speed.replaceChildren(
      new Option('None', 'none'),
      new Option('Low', 'low'),
      new Option('Mid', 'medium'),
      new Option('High', 'high'),
    );
    speed.value = 'none';
    speed.id = 'answerThinking';
    speed.title = 'GLM 使用 thinking.type；Codex 使用对应速度挡位';
  }
  const question = $('question');
  if (question) {
    // Keep the conversation in the familiar order: previous replies first,
    // then the composer at the bottom.  The static shell also remains
    // backwards-compatible because this is a DOM move, not a new API.
    const composer = question.closest('.chat-composer');
    if (composer && output && composer.parentElement === output.parentElement) composer.before(output);
    if (!$('answerRefs')) {
      const tray = document.createElement('div');
      tray.id = 'answerRefs';
      tray.className = 'citation-tray';
      question.before(tray);
    }
    question.placeholder = '例如：这段剧情讲了什么？\nEnter 发送，Shift+Enter 换行';
    question.defaultValue = '';
    question.onkeydown = (event) => {
      if (event.key === 'Enter' && !event.shiftKey) {
        event.preventDefault();
        answer();
      }
    };
  }
  renderAnswerRefs();
  document.querySelector('[data-view="reader"]')?.addEventListener('click', loadSessions);
  loadSessions();
  if (output && !output.nextElementSibling?.classList.contains('chat-status')) {
    const status = document.createElement('div');
    status.id = 'chatStatus';
    status.className = 'chat-status';
    status.textContent = '可连续追问；每次回答都会保留引用和用量。';
    output.after(status);
  }
}

function createSessionPanel() {
  const layout = document.querySelector('.reader-layout');
  if (!layout || $('sessionPanel')) return;
  const panel = document.createElement('article');
  panel.id = 'sessionPanel';
  panel.className = 'panel session-panel';
  panel.innerHTML = '<div class="panel-heading"><div><span class="eyebrow">HISTORY</span><h3>过往对话</h3></div><div class="session-panel-actions"><button class="text-button" id="sessionsToggle" type="button">收起</button><button class="text-button" id="sessionsRefresh" type="button">刷新</button></div></div><button class="button secondary full" id="newSession">＋ 新对话</button><div id="sessionList" class="session-list"><div class="empty-state">还没有保存的对话</div></div>';
  layout.prepend(panel);
  $('sessionsRefresh').onclick = loadSessions;
  $('newSession').onclick = newSession;
  const toggle = $('sessionsToggle');
  let collapsed = false;
  try { collapsed = window.localStorage.getItem('readtrace.historyCollapsed') === 'true'; } catch (_) { /* private browsing */ }
  const apply = (value) => {
    collapsed = value;
    layout.classList.toggle('history-collapsed', collapsed);
    toggle.textContent = collapsed ? '展开' : '收起';
    toggle.title = collapsed ? '展开过往对话' : '收起过往对话';
    toggle.setAttribute('aria-label', toggle.title);
    toggle.setAttribute('aria-pressed', String(collapsed));
  };
  apply(collapsed);
  toggle.onclick = () => {
    apply(!collapsed);
    try { window.localStorage.setItem('readtrace.historyCollapsed', String(collapsed)); } catch (_) { /* private browsing */ }
  };
}

function renderSessions() {
  const list = $('sessionList');
  if (!list) return;
  list.replaceChildren();
  if (!state.sessions?.length) {
    list.className = 'session-list empty-state';
    list.textContent = '还没有保存的对话';
    return;
  }
  list.className = 'session-list';
  state.sessions.forEach((session) => {
    const button = document.createElement('button');
    button.className = `session-item${session.session_id === state.answerSessionId ? ' active' : ''}`;
    const title = document.createElement('strong');
    title.textContent = session.title || '新对话';
    const meta = document.createElement('small');
    meta.textContent = `${session.status === 'completed' ? '已完成' : session.status || '进行中'} · ${session.message_count || 0} 条消息`;
    const date = document.createElement('span');
    date.textContent = session.updated_at ? new Date(session.updated_at).toLocaleString() : '';
    button.append(title, meta, date);
    button.onclick = () => openSession(session.session_id);
    list.append(button);
  });
}

async function loadSessions() {
  const result = await api('/api/sessions');
  if (!result.ok) return;
  state.sessions = result.sessions || [];
  renderSessions();
}

async function openSession(sessionId) {
  const result = await api(`/api/sessions/${encodeURIComponent(sessionId)}`);
  if (!result.ok) return toast(result.error || '读取对话失败', true);
  const session = result.session;
  state.answerSessionId = session.session_id;
  state.chatMessages = (session.messages || [])
    .filter((message) => ['user', 'assistant', 'error'].includes(message.role))
    .map((message) => ({ role: message.role, content: message.content, source_refs: message.source_refs || [] }));
  state.answerRefs = new Set((session.messages || []).flatMap((message) => message.source_refs || []));
  state.answerQuotes = new Map();
  renderChat();
  renderAnswerRefs();
  const status = $('chatStatus');
  if (status) status.textContent = `会话 ${state.answerSessionId} · 已恢复，可继续追问`;
  renderSessions();
}

function newSession() {
  state.answerSessionId = null;
  state.chatMessages = [];
  state.answerRefs = new Set();
  state.answerQuotes = new Map();
  renderChat();
  renderAnswerRefs();
  const status = $('chatStatus');
  if (status) status.textContent = '新对话；选择引用后即可提问。';
  renderSessions();
}

function collectAnswerRefs() {
  const refs = new Set(state.answerRefs || []);
  [...state.selectedUnits].forEach((id) => {
    const unit = state.units.find((item) => item.unit_id === id);
    if (unit?.kind === 'clean') (unit.source_refs || []).forEach((ref) => refs.add(ref));
  });
  return refs;
}

function collectAnswerQuotes() {
  return [...(state.answerQuotes || new Map()).values()]
    .map((quote) => quote.text)
    .filter(Boolean);
}

function renderAnswerRefs() {
  const tray = $('answerRefs');
  if (!tray) return;
  const refs = [...collectAnswerRefs()];
  const quotes = [...(state.answerQuotes || new Map()).values()];
  tray.replaceChildren();
  const label = document.createElement('span');
  label.className = 'citation-label';
  const count = refs.length + quotes.length;
  label.textContent = count ? `已引用 ${count} 条` : '引用内容（可选）';
  tray.append(label);
  const pickerCount = $('citationSelectionCount');
  if (pickerCount) pickerCount.textContent = `已选 ${quotes.length} 个文件`;
  if (!count) {
    const hint = document.createElement('small');
    hint.textContent = '在 clean 文件树中选择文件，或从 clean 搜索结果加入引用';
    tray.append(hint);
    return;
  }
  refs.forEach((ref) => {
    const chip = document.createElement('span');
    chip.className = 'citation-chip';
    chip.title = ref;
    chip.textContent = ref.split('/').pop() || ref;
    if (state.answerRefs?.has(ref)) {
      const remove = document.createElement('button');
      remove.type = 'button';
      remove.setAttribute('aria-label', `移除引用 ${ref}`);
      remove.textContent = '×';
      remove.onclick = () => { state.answerRefs.delete(ref); renderAnswerRefs(); };
      chip.append(remove);
    }
    tray.append(chip);
  });
  quotes.forEach((quote) => {
    const chip = document.createElement('span');
    chip.className = 'citation-chip file-quote-chip';
    chip.title = quote.path;
    chip.textContent = quote.path.split('/').pop() || quote.path;
    const remove = document.createElement('button');
    remove.type = 'button';
    remove.setAttribute('aria-label', `移除文件引用 ${quote.path}`);
    remove.textContent = '×';
    remove.onclick = () => { state.answerQuotes.delete(quote.path); renderAnswerRefs(); renderCitationFileList(); };
    chip.append(remove);
    tray.append(chip);
  });
}

function renderChat() {
  const box = $('answerOut');
  if (!box) return;
  box.replaceChildren();
  if (!state.chatMessages?.length) {
    box.className = 'chat-history empty-state';
    box.textContent = '选择 clean 文件后，在下方输入问题开始对话。';
    return;
  }
  box.className = 'chat-history';
  state.chatMessages.forEach((message) => {
    const row = document.createElement('div');
    row.className = `chat-message ${message.role || 'assistant'}`;
    const bubble = document.createElement('div');
    bubble.className = 'chat-bubble';
    bubble.textContent = message.content || '';
    row.append(bubble);
    const meta = [];
    if (message.source_refs?.length) meta.push(`引用 ${message.source_refs.length} 条`);
    if (message.quote_count) meta.push(`文件引用 ${message.quote_count} 个`);
    if (message.usage?.total_tokens != null) meta.push(`Token ${Number(message.usage.total_tokens).toLocaleString()}`);
    if (message.usage?.cost_usd != null) meta.push(`$${Number(message.usage.cost_usd).toFixed(6)}`);
    if (meta.length) {
      const details = document.createElement('div');
      details.className = 'chat-meta';
      details.textContent = meta.join(' · ');
      bubble.append(details);
    }
    box.append(row);
  });
  box.scrollTop = box.scrollHeight;
}

async function search() {
  const query = $('query').value.trim();
  if (!query) return toast('先输入关键词', true);
  const result = await api(`/api/search?q=${encodeURIComponent(query)}`);
  const box = $('searchOut');
  if (!result.ok) return (box.textContent = result.error || '搜索失败');
  const hits = result.hits || [];
  box.className = hits.length ? 'search-results' : 'search-results empty-state';
  box.replaceChildren();
  hits.forEach((hit) => {
    const row = document.createElement('div');
    row.className = 'search-hit';
    const head = document.createElement('div');
    head.className = 'search-hit-head';
    const path = document.createElement('strong');
    path.textContent = hit.path || hit.source_ref || '命中';
    const line = document.createElement('span');
    line.textContent = `第 ${hit.line || '—'} 行`;
    head.append(path, line);
    const context = document.createElement('pre');
    context.className = 'search-context';
    context.textContent = (hit.context?.length ? hit.context : [hit.snippet || hit.text || '']).join('\n');
    row.append(head, context);
    if (hit.source_refs?.length) {
      const actions = document.createElement('div');
      actions.className = 'search-hit-actions';
      const refs = document.createElement('small');
      refs.textContent = `可引用 ${hit.source_refs.length} 个来源`;
      const quote = document.createElement('button');
      quote.className = 'text-button';
      quote.textContent = '加入引用';
      quote.onclick = () => {
        hit.source_refs.forEach((ref) => state.answerRefs.add(ref));
        renderAnswerRefs();
        toast(`已加入 ${hit.source_refs.length} 个引用`);
      };
      actions.append(refs, quote);
      row.append(actions);
    }
    box.append(row);
  });
  if (!hits.length) box.textContent = '没有找到匹配内容';
}

async function answer() {
  const question = $('question').value.trim();
  if (!question) return toast('先写下问题', true);
  const refs = new Set(state.answerRefs || []);
  [...state.selectedUnits].forEach((id) => {
    const unit = state.units.find((item) => item.unit_id === id);
    if (unit?.kind === 'clean') (unit.source_refs || []).forEach((ref) => refs.add(ref));
  });
  const button = $('answer');
  if (button) button.disabled = true;
  state.chatMessages.push({ role: 'user', content: question, source_refs: [...refs] });
  renderChat();
  const status = $('chatStatus');
  if (status) status.textContent = '正在询问模型…';
  try {
    const result = await post('/api/answer', {
      query: question,
      provider: $('answerProvider').value,
      thinking: $('answerThinking')?.value || 'none',
      source_refs: [...refs],
      session_id: state.answerSessionId,
    });
    if (!result.ok) {
      state.chatMessages.push({ role: 'error', content: result.error || '问答失败' });
      if (status) status.textContent = '本轮失败，可以修改选项后重试。';
      renderChat();
      return;
    }
    state.answerSessionId = result.session_id || state.answerSessionId;
    state.chatMessages.push({
      role: 'assistant',
      content: result.answer || '模型未返回答案。',
      source_refs: result.source_refs || [],
      usage: result.usage,
    });
    $('question').value = '';
    if (status) status.textContent = `会话 ${state.answerSessionId || '—'} · 可继续追问`;
    renderChat();
    await loadSessions();
    await loadUsage();
  } catch (error) {
    state.chatMessages.push({ role: 'error', content: error.message || '网络请求失败' });
    if (status) status.textContent = '网络请求失败，可以重试。';
    renderChat();
  } finally {
    if (button) button.disabled = false;
  }
}

prepareReaderUi();
renderChat();

// Provider desk: profiles are metadata plus a local key handle.  The server
// never sends the key itself; the empty password field intentionally preserves
// the stored value on save.
state.providers = [];
state.currentProvider = null;
function providerById(id) { return state.providers.find((profile) => profile.id === id); }
function defaultProvider() {
  return state.providers.find((profile) => profile.enabled && profile.model === 'glm-5.2')
    || state.providers.find((profile) => profile.enabled)
    || null;
}
function providerShortName(profile) {
  if (!profile) return '—';
  if (profile.id === 'codex-luna') return 'Codex Luna';
  if (profile.id === 'mock') return 'Mock';
  if (profile.model === 'glm-5.3-flash') return 'GLM-5.3 Flash';
  if (profile.model === 'glm-5.2') return 'GLM-5.2';
  return profile.model || profile.name;
}
function renderProviderSelect(id) {
  const select = $(id);
  if (!select) return;
  const current = select.value;
  select.replaceChildren();
  state.providers.filter((profile) => profile.enabled).forEach((profile) => {
    select.add(new Option(providerShortName(profile), profile.id));
  });
  if (providerById(current)?.enabled) select.value = current;
  else if (defaultProvider()) select.value = defaultProvider().id;
}
function fillProviderForm(profile) {
  state.currentProvider = profile || null;
  $('providerEditorTitle').textContent = profile ? profile.name : '新建来源';
  $('providerId').value = profile?.builtin ? '' : profile?.id || '';
  $('providerName').value = profile?.name || '';
  $('providerKind').value = profile?.kind || 'http';
  $('providerModel').value = profile?.model || '';
  $('providerThinking').value = profile?.thinking_mode || 'none';
  $('providerBaseUrl').value = profile?.base_url || '';
  $('providerEndpoint').value = profile?.endpoint || '';
  $('providerEndpointPath').value = profile?.endpoint_path || 'chat/completions';
  $('providerKeyEnv').value = profile?.api_key_env || '';
  $('providerKey').value = '';
  $('providerAuthHeader').value = profile?.auth_header || 'Authorization';
  $('providerAuthScheme').value = profile?.auth_scheme || 'Bearer';
  $('providerMaxTokens').value = profile?.max_tokens_field || 'max_tokens';
  $('providerResponseFormat').value = profile?.response_format || 'json_object';
  $('providerInputPrice').value = profile?.input_price_per_million ?? 0;
  $('providerCachedPrice').value = profile?.cached_input_price_per_million ?? 0;
  $('providerOutputPrice').value = profile?.output_price_per_million ?? 0;
  $('providerPricingVersion').value = profile?.pricing_version || '';
  $('providerEnabled').checked = profile?.enabled ?? true;
  const badge = $('providerKeyBadge');
  if (badge) badge.textContent = profile?.key_present ? 'KEY 已配置' : 'KEY 未配置';
  const clear = $('providerClearKey');
  if (clear) clear.checked = false;
  $('providerDelete').disabled = !profile || profile.builtin;
}
function renderProviderList() {
  const box = $('providerList');
  if (!box) return;
  box.replaceChildren();
  if (!state.providers.length) {
    box.className = 'provider-list empty-state';
    box.textContent = '还没有来源';
    return;
  }
  box.className = 'provider-list';
  state.providers.forEach((profile) => {
    const button = document.createElement('button');
    button.className = `provider-item${state.currentProvider?.id === profile.id ? ' active' : ''}`;
    const title = document.createElement('strong');
    title.textContent = profile.name;
    const details = document.createElement('span');
    details.textContent = `${profile.kind} · ${profile.model}`;
    const badges = document.createElement('small');
    badges.textContent = `${profile.key_present ? 'Key 已配置' : '无 Key'}${profile.builtin ? ' · 内置' : ''}${profile.enabled ? '' : ' · 已停用'}`;
    button.append(title, details, badges);
    button.onclick = () => { fillProviderForm(profile); renderProviderList(); };
    box.append(button);
  });
}
async function loadProviders(selectId) {
  const result = await api('/api/providers');
  if (!result.ok) return toast(result.error || '来源加载失败', true);
  state.providers = result.profiles || [];
  if (selectId) state.currentProvider = providerById(selectId) || state.currentProvider;
  if (!state.currentProvider) state.currentProvider = defaultProvider();
  ['provider', 'queueProvider', 'answerProvider'].forEach(renderProviderSelect);
  renderProviderList();
  fillProviderForm(state.currentProvider);
  const hint = $('providerStoreHint');
  if (hint) hint.textContent = `本机配置存储：${result.store || '用户配置目录'}（不会进入项目 Git）`;
}
function profileFormBody() {
  const body = {
    id: $('providerId').value.trim() || undefined,
    name: $('providerName').value.trim(),
    kind: $('providerKind').value,
    model: $('providerModel').value.trim(),
    base_url: $('providerBaseUrl').value.trim() || undefined,
    endpoint: $('providerEndpoint').value.trim() || undefined,
    endpoint_path: $('providerEndpointPath').value.trim() || 'chat/completions',
    api_key_env: $('providerKeyEnv').value.trim() || undefined,
    api_key: $('providerKey').value || undefined,
    clear_api_key: $('providerClearKey')?.checked || false,
    auth_header: $('providerAuthHeader').value.trim() || 'Authorization',
    auth_scheme: $('providerAuthScheme').value.trim() || 'Bearer',
    max_tokens_field: $('providerMaxTokens').value.trim() || 'max_tokens',
    response_format: $('providerResponseFormat').value,
    thinking_mode: $('providerThinking').value,
    input_price_per_million: Number($('providerInputPrice').value || 0),
    cached_input_price_per_million: Number($('providerCachedPrice').value || 0),
    output_price_per_million: Number($('providerOutputPrice').value || 0),
    pricing_version: $('providerPricingVersion').value.trim() || undefined,
    enabled: $('providerEnabled').checked,
  };
  return body;
}
async function saveProvider(event) {
  event.preventDefault();
  if (!$('providerName').value.trim() || !$('providerModel').value.trim()) return toast('名称和模型不能为空', true);
  const result = await post('/api/providers', profileFormBody());
  if (!result.ok) return toast(result.error || '保存来源失败', true);
  toast('来源已保存；Key 不会返回网页或进入 Git');
  await loadProviders(result.profile?.id);
}
async function checkProvider() {
  const id = $('providerId').value.trim() || state.currentProvider?.id;
  if (!id) return toast('先保存这个来源', true);
  $('providerOut').textContent = '正在测试连接…';
  const result = await post('/api/providers/check', { id });
  show('providerOut', result);
  toast(result.ok ? '连接测试成功' : (result.error || result.report?.error || '连接测试失败'), !result.ok);
}
async function deleteProvider() {
  const id = $('providerId').value.trim();
  if (!id || state.currentProvider?.builtin) return toast('内置来源不能删除', true);
  if (!window.confirm(`删除来源 ${id}？不会删除 Vault 素材。`)) return;
  const result = await post('/api/providers/delete', { id });
  if (!result.ok) return toast(result.error || '删除失败', true);
  toast('来源已删除');
  await loadProviders();
}
function prepareProviderUi() {
  titles.providers = '来源与 API';
  const providerIntro = document.querySelector('#view-providers .view-intro p');
  if (providerIntro) providerIntro.textContent = '统一管理清华 GLM、Codex、Mock 和自定义 OpenAI-compatible 来源。密钥只保存在本机配置；受限环境会回退到当前 Vault 的 .readtrace，接口不会把密钥返回给网页。';
  const key = $('providerKey');
  if (key && !$('providerClearKey')) {
    const label = document.createElement('label');
    label.className = 'check-label provider-clear-key';
    label.innerHTML = '<input id="providerClearKey" type="checkbox"> 清除已保存的 Key';
    key.closest('label')?.after(label);
  }
  $('providerForm')?.addEventListener('submit', saveProvider);
  $('providerNew')?.addEventListener('click', () => fillProviderForm(null));
  $('providerRefresh')?.addEventListener('click', () => loadProviders());
  $('providerCheck')?.addEventListener('click', checkProvider);
  $('providerDelete')?.addEventListener('click', deleteProvider);
  document.querySelector('[data-view="providers"]')?.addEventListener('click', () => loadProviders());
  loadProviders();
}
const originalLlmBody = llmBody;
llmBody = function profileLlmBody(provider) {
  const body = originalLlmBody(provider);
  if (!body) return null;
  const profile = providerById(provider);
  if (profile) {
    body.profile_id = profile.id;
    body.provider = profile.kind;
  }
  return body;
};
async function runQueuedPipeline(batchId, item) {
  const ocr = await post('/api/ocr', { batch_id: batchId, provider: item.ocr });
  if (!ocr.ok) throw new Error(ocr.error);
  const ocrTask = await waitTask(ocr.task_id);
  if (ocrTask.status !== 'completed') throw new Error(ocrTask.error || 'OCR 失败');
  const normalized = await post('/api/normalize', { batch_id: batchId });
  if (!normalized.ok) throw new Error(normalized.error);
  const profile = providerById(item.provider);
  const repair = await post('/api/repair', {
    batch_id: batchId,
    profile_id: profile?.id || item.provider,
    provider: profile?.kind,
    speed: item.speed,
    ...(item.model ? { model: item.model } : {}),
  });
  if (!repair.ok) throw new Error(repair.error);
  const repairTask = await waitTask(repair.task_id);
  if (repairTask.status !== 'completed') throw new Error(repairTask.error || 'LLM 修复失败');
  const merged = await post('/api/merge', { batch_id: batchId, confirm: true, ...(item.cleanName ? { clean_name: item.cleanName } : {}) });
  if (!merged.ok) throw new Error(merged.error);
  return merged;
}
answer = async function profileAnswer() {
  const question = $('question').value.trim();
  if (!question) return toast('先写下问题', true);
  const refs = collectAnswerRefs();
  const quotes = collectAnswerQuotes();
  renderAnswerRefs();
  const button = $('answer');
  if (button) button.disabled = true;
   state.chatMessages.push({ role: 'user', content: question, source_refs: [...refs], quote_count: quotes.length });
  renderChat();
  const status = $('chatStatus');
  if (status) status.textContent = '正在询问模型…';
  try {
    const profile = providerById($('answerProvider').value);
    const result = await post('/api/answer', {
      query: question,
      profile_id: profile?.id,
      provider: profile?.kind,
      thinking: $('answerThinking')?.value || 'none',
      source_refs: [...refs],
      quotes,
      session_id: state.answerSessionId,
    });
    if (!result.ok) {
      state.chatMessages.push({ role: 'error', content: result.error || '问答失败' });
      if (status) status.textContent = '本轮失败，可以修改来源或挡位后重试。';
      renderChat();
      return;
    }
    state.answerSessionId = result.session_id || state.answerSessionId;
    state.chatMessages.push({ role: 'assistant', content: result.answer || '模型未返回答案。', source_refs: result.source_refs || [], usage: result.usage });
    $('question').value = '';
    if (status) status.textContent = `会话 ${state.answerSessionId || '—'} · 可继续追问`;
    renderChat();
    await loadSessions();
    await loadUsage();
  } catch (error) {
    state.chatMessages.push({ role: 'error', content: error.message || '网络请求失败' });
    if (status) status.textContent = '网络请求失败，可以重试。';
    renderChat();
  } finally {
    if (button) button.disabled = false;
  }
};
$('answer').onclick = answer;
prepareProviderUi();

// Keep selectors compact and consistent across import, processing, provider
// configuration, and chat. Values remain the protocol values used by the API.
function normalizeSelectorLabels() {
  const replace = (id, options) => {
    const select = $(id);
    if (!select) return;
    const current = select.value;
    select.replaceChildren(...options.map(([label, value]) => new Option(label, value)));
    select.value = options.some(([, value]) => value === current) ? current : options[0][1];
  };
  replace('queueSpeed', [['Low', 'low'], ['Mid', 'mid'], ['High', 'high']]);
  replace('speed', [['Low', 'low'], ['Mid', 'mid'], ['High', 'high']]);
  replace('answerThinking', [['None', 'none'], ['Low', 'low'], ['Mid', 'medium'], ['High', 'high']]);
  replace('providerThinking', [['None', 'none'], ['Low', 'low'], ['Mid', 'medium'], ['High', 'high']]);
}

function prepareSearchAndChatPages() {
  const reader = $('view-reader');
  const searchPanel = reader?.querySelector('.search-panel');
  if (reader && searchPanel && !$('view-search')) {
    const searchView = document.createElement('section');
    searchView.id = 'view-search';
    searchView.className = 'view';
    searchView.innerHTML = '<div class="view-intro"><div><span class="eyebrow">LOCAL SEARCH</span><h2>检索</h2><p>只查询本地索引，结果直接显示上下文；需要对话时，在阅读与问答页添加引用。</p></div></div>';
    const layout = document.createElement('div');
    layout.className = 'search-page-layout';
    layout.append(searchPanel);
    searchView.append(layout);
    reader.parentElement.insertBefore(searchView, reader);
    const intro = reader.querySelector('.view-intro p');
    if (intro) intro.textContent = '';
  }
  const nav = document.querySelector('.nav-tabs');
  const readerTab = nav?.querySelector('[data-view="reader"]');
  if (nav && readerTab && !nav.querySelector('[data-view="search"]')) {
    const tab = document.createElement('button');
    tab.className = 'nav-tab';
    tab.dataset.view = 'search';
    tab.innerHTML = '<span>⌕</span>检索';
    tab.onclick = () => go('search');
    nav.insertBefore(tab, readerTab);
  }
}

function renderCitationPickerResults(box, hits) {
  box.replaceChildren();
  if (!hits.length) {
    box.className = 'citation-picker-results empty-state';
    box.textContent = '没有找到可引用内容';
    return;
  }
  box.className = 'citation-picker-results';
  hits.forEach((hit) => {
    const row = document.createElement('article');
    row.className = 'citation-picker-hit';
    const head = document.createElement('strong');
    head.textContent = hit.path || hit.source_ref || '命中';
    const context = document.createElement('pre');
    context.textContent = (hit.context?.length ? hit.context : [hit.snippet || hit.text || '']).join('\n');
    const action = document.createElement('button');
    action.className = 'button secondary';
    action.type = 'button';
    action.textContent = '添加引用';
    action.onclick = () => {
      const refs = [...new Set(hit.source_refs || [])];
      refs.forEach((ref) => state.answerRefs.add(ref));
      renderAnswerRefs();
      toast(`已添加 ${refs.length} 条引用`);
    };
    row.append(head, context, action);
    box.append(row);
  });
}

async function citationPickerSearch(input, box) {
  const query = input.value.trim();
  if (!query) {
    box.className = 'citation-picker-results empty-state';
    box.textContent = '输入关键词查找可引用内容';
    return;
  }
  const sequence = ++state.citationSearchSeq;
  if (state.citationSearchCache.has(query)) {
    renderCitationPickerResults(box, state.citationSearchCache.get(query));
    return;
  }
  box.className = 'citation-picker-results empty-state';
  box.textContent = '正在检索…';
  const result = await api(`/api/search?q=${encodeURIComponent(query)}`);
  if (sequence !== state.citationSearchSeq || input.value.trim() !== query) return;
  if (!result.ok) {
    box.textContent = result.error || '检索失败';
    return;
  }
  state.citationSearchCache.set(query, result.hits || []);
  renderCitationPickerResults(box, result.hits || []);
}

function citationFileEligible(file) {
  if (!file || file.kind !== 'text') return false;
  const path = String(file.path || '').replaceAll('\\', '/');
  return path.startsWith('clean/') && /\.(md|txt)$/i.test(path);
}

function renderCitationFileList() {
  const box = $('citationFileList');
  if (!box) return;
  const filter = ($('citationFileFilter')?.value || '').trim().toLowerCase();
  const files = (state.citationFiles || [])
    .filter(citationFileEligible)
    .filter((file) => !filter || `${file.name} ${file.path}`.toLowerCase().includes(filter));
  box.replaceChildren();
  if (!files.length) {
    box.className = 'citation-file-list empty-state';
    box.textContent = filter ? '没有匹配的 clean 文件' : '还没有可引用的 clean 文件；先生成或整理一份 Markdown。';
    return;
  }
  box.className = 'citation-file-list';
  const root = { dirs: new Map(), files: [] };
  files.forEach((file) => {
    const parts = file.path.split('/');
    let node = root;
    parts.slice(0, -1).forEach((part) => {
      if (!node.dirs.has(part)) node.dirs.set(part, { dirs: new Map(), files: [] });
      node = node.dirs.get(part);
    });
    node.files.push(file);
  });
  if (!state.citationExpandedDirs.size) {
    files.forEach((file) => {
      const parts = file.path.split('/');
      for (let i = 1; i < parts.length; i += 1) state.citationExpandedDirs.add(parts.slice(0, i).join('/'));
    });
  }
  const draw = (node, prefix, depth) => {
    [...node.dirs.keys()].sort((a, b) => a.localeCompare(b, 'zh-CN')).forEach((name) => {
      const key = prefix ? `${prefix}/${name}` : name;
      const open = state.citationExpandedDirs.has(key);
      const folder = document.createElement('button');
      folder.type = 'button';
      folder.className = 'citation-folder-row';
      folder.style.paddingLeft = `${8 + depth * 14}px`;
      folder.innerHTML = `<span>${open ? '⌄' : '›'}</span><strong>${escapeHtml(name)}</strong>`;
      folder.onclick = () => { if (open) state.citationExpandedDirs.delete(key); else state.citationExpandedDirs.add(key); renderCitationFileList(); };
      box.append(folder);
      if (open) draw(node.dirs.get(name), key, depth + 1);
    });
    [...node.files].sort((a, b) => (a.name || a.path).localeCompare(b.name || b.path, 'zh-CN')).forEach((file) => {
      const row = document.createElement('label');
      row.className = 'citation-file-item';
      row.style.paddingLeft = `${8 + depth * 14}px`;
      const check = document.createElement('input');
      check.type = 'checkbox';
      check.checked = state.answerQuotes?.has(file.path) || false;
      check.onchange = async () => {
        if (!check.checked) {
          state.answerQuotes.delete(file.path);
          renderAnswerRefs();
          return;
        }
        check.disabled = true;
        const result = await api(`/api/file?path=${encodeURIComponent(file.path)}`);
        check.disabled = false;
        if (!result.ok) { check.checked = false; return toast(result.error || '读取引用文件失败', true); }
        const text = String(result.content || '').trim();
        if (!text) { check.checked = false; return toast('该文件没有可引用的文本', true); }
        const maxChars = 50000;
        state.answerQuotes.set(file.path, { path: file.path, text: text.length > maxChars ? `${text.slice(0, maxChars)}\n[文件过长，引用已截断]` : text });
        renderAnswerRefs();
      };
      const text = document.createElement('span');
      text.className = 'citation-file-copy';
      const title = document.createElement('strong');
      title.textContent = file.name || file.path;
      const path = document.createElement('small');
      path.textContent = file.path;
      text.append(title, path);
      row.append(check, text);
      box.append(row);
    });
  };
  draw(root, '', 0);
  const count = $('citationSelectionCount');
  if (count) count.textContent = `已选 ${state.answerQuotes?.size || 0} 个文件`;
}

async function loadCitationFiles() {
  const box = $('citationFileList');
  if (!box) return;
  box.className = 'citation-file-list empty-state';
  box.textContent = '正在读取文件…';
  const result = await api('/api/files?view=essential');
  if (!result.ok) {
    box.textContent = result.error || '读取文件失败';
    return;
  }
  state.citationFiles = result.files || [];
  state.citationFilesLoaded = true;
  state.citationSearchCache.clear();
  renderCitationFileList();
}

function openCitationPicker() {
  let dialog = $('citationDialog');
  if (!dialog) {
    dialog = document.createElement('dialog');
    dialog.id = 'citationDialog';
    dialog.className = 'citation-dialog';
    dialog.innerHTML = '<form method="dialog" class="citation-picker"><div class="dialog-heading"><div><span class="eyebrow">ADD CITATION</span><h3>添加引用</h3></div><button value="cancel" class="icon-button" aria-label="关闭">×</button></div><input id="citationQuery" class="citation-picker-input" placeholder="搜索 clean 文件内容" autocomplete="off"><div id="citationPickerResults" class="citation-picker-results empty-state">输入关键词查找可引用内容</div><div class="citation-files-heading"><strong>Clean 文件</strong><button id="citationFilesRefresh" type="button" class="text-button">刷新</button></div><input id="citationFileFilter" class="citation-file-filter" placeholder="筛选文件名或路径（本地即时）" autocomplete="off"><div id="citationFileList" class="citation-file-list empty-state">打开时读取 clean 文件</div><div class="dialog-actions"><span id="citationSelectionCount" class="muted">已选 0 个文件</span><button value="cancel" class="button secondary">完成</button></div></form>';
    document.body.append(dialog);
    const input = $('citationQuery');
    const box = $('citationPickerResults');
    let timer;
    input.oninput = () => {
      clearTimeout(timer);
      timer = setTimeout(() => citationPickerSearch(input, box), 180);
    };
    input.onkeydown = (event) => {
      if (event.key === 'Enter') {
        event.preventDefault();
        citationPickerSearch(input, box);
      }
    };
    $('citationFilesRefresh').onclick = loadCitationFiles;
    $('citationFileFilter').oninput = renderCitationFileList;
  }
  dialog.showModal();
  $('citationQuery').value = '';
  $('citationFileFilter').value = '';
  $('citationPickerResults').className = 'citation-picker-results empty-state';
  $('citationPickerResults').textContent = '输入关键词查找可引用内容';
  // Refresh the small clean projection on every open so switching Vaults
  // never leaves files from the previous Vault in the picker.
  loadCitationFiles();
  $('citationQuery').focus();
}

function prepareCitationPicker() {
  const tray = $('answerRefs');
  if (!tray || $('addCitation')) return;
  const button = document.createElement('button');
  button.id = 'addCitation';
  button.type = 'button';
  button.className = 'button secondary citation-add';
  button.textContent = '＋ 添加引用';
  button.onclick = openCitationPicker;
  tray.before(button);
}

const originalPreviewFile = previewFile;
previewFile = async function editablePreviewFile(file) {
  state.currentFile = file;
  renderFiles();
  $('previewTitle').textContent = file.name;
  const box = $('previewContent');
  if (file.kind === 'image' || file.kind === 'pdf') return originalPreviewFile(file);
  const result = await api(`/api/file?path=${encodeURIComponent(file.path)}`);
  if (!result.ok) {
    box.textContent = result.error || '读取文件失败';
    return;
  }
  const isMarkdown = file.name.toLowerCase().endsWith('.md');
  if (isMarkdown) {
    const toolbar = document.createElement('div');
    toolbar.className = 'preview-edit-toolbar';
    const hint = document.createElement('span');
    hint.textContent = result.truncated ? '内容过长，只读预览' : 'Markdown 可编辑';
    const save = document.createElement('button');
    save.id = 'savePreview';
    save.className = 'button primary';
    save.type = 'button';
    save.textContent = '保存';
    save.disabled = Boolean(result.truncated);
    toolbar.append(hint, save);
    const editor = document.createElement('textarea');
    editor.id = 'previewEditor';
    editor.className = 'preview-editor';
    editor.value = result.content || '';
    editor.readOnly = Boolean(result.truncated);
    box.replaceChildren(toolbar, editor);
    save.onclick = async () => {
      save.disabled = true;
      const response = await post('/api/file', { path: file.path, content: editor.value });
      if (!response.ok) {
        toast(response.error || '保存失败', true);
        save.disabled = false;
        return;
      }
      toast('文件已保存，搜索索引已刷新');
      const current = state.files.find((item) => item.path === file.path);
      if (current) current.size = response.size || current.size;
      save.disabled = false;
    };
    return;
  }
  box.innerHTML = `<pre class="code-preview">${escapeHtml(result.content || '该文件没有可显示的文本内容。')}</pre>${result.truncated ? '<small class="muted">内容过长，仅显示前 120,000 个字符。</small>' : ''}`;
};

normalizeSelectorLabels();
prepareSearchAndChatPages();
prepareCitationPicker();
prepareSidebarUi();
prepareRepairPromptUi();
prepareFilePickerUi();
runQueue = async function enhancedRunQueue() {
  if (!state.queue.length) return;
  const items = [...state.queue];
  state.queue = [];
  renderQueue();
  const results = [];
  let previewBatch = null;
  let firstBatch = null;
  let firstCleanName = '';
  for (const item of items) {
    try {
      const result = item.uploadFiles?.length
        ? await uploadFiles(item.uploadFiles, { mode: item.mode, order: 'filename' })
        : await post('/api/import', { path: item.path, mode: item.mode, no_copy: !item.copy });
      if (!result.ok) throw new Error(result.error);
      const batchId = result.batch.batch_id;
      results.push(batchId);
      if (!firstBatch) { firstBatch = batchId; firstCleanName = item.cleanName || ''; }
      if (item.merge === 'direct') {
        const direct = await post('/api/direct-clean', {
          batch_id: batchId,
          ...(item.cleanName ? { clean_name: item.cleanName } : {}),
        });
        if (!direct.ok) throw new Error(direct.error || 'TXT/MD 直接发布失败');
        toast(`已直接发布到 ${direct.clean_path || 'clean/'}`);
      } else if (item.merge === 'auto') await runQueuedPipeline(batchId, item);
      if (item.merge === 'preview' && !previewBatch) { previewBatch = batchId; firstCleanName = item.cleanName || ''; }
      toast(`已导入 ${batchId}`);
    } catch (error) {
      results.push(`失败：${error.message}`);
      toast(error.message, true);
    }
  }
  show('importOut', { batches: results });
  await refresh();
  const first = firstBatch || results.find((value) => !value.startsWith('失败'));
  if (previewBatch || ($('afterImport').value === 'process' && first)) {
    $('batch').value = previewBatch || first;
    $('cleanName').value = firstCleanName;
    go('process');
  } else go('files');
};
$('queueRun').onclick = runQueue;
mergeUnits = async function enhancedMergeUnits(confirm) {
  if (!confirm && state.selectedUnits.size < 1) return toast('至少选择一个 source 或 clean 单元', true);
  const allowUnrepaired = $('allowUnrepaired')?.checked || $('allowUnrepairedUnits')?.checked || false;
  const cleanName = $('mergeCleanName')?.value.trim() || $('cleanName')?.value.trim() || undefined;
  const body = confirm
    ? { confirm: true, merge_id: state.pendingUnitMerge?.merge_id, allow_unrepaired: allowUnrepaired, clean_name: cleanName }
    : { confirm: false, units: [...state.selectedUnits], allow_unrepaired: allowUnrepaired, clean_name: cleanName };
  if (confirm && !body.merge_id) return toast('请先预览合并计划', true);
  const result = await post('/api/merge-units', body);
  if (!result.ok) return toast(result.error, true);
  state.pendingUnitMerge = result.plan;
  $('mergeConfirmUnits').disabled = !result.confirmation_required;
  if (result.confirmation_required) {
    $('previewTitle').textContent = '合并预览';
    $('previewContent').innerHTML = `<pre class="code-preview">${escapeHtml(pretty(result.plan))}</pre>`;
  } else {
    const warning = result.warning || '';
    toast(result.clean_path ? `已发布到 ${result.clean_path}` : (warning || '跨 batch revision 已生成'));
    await loadFiles();
  }
};
prepareQueueCleanNameUi();
