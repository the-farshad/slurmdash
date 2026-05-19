// slurmdash web — minimal vanilla JS dashboard.
// Polls /api/dashboard every REFRESH_MS, renders the four panels, and
// proxies destructive actions back through /api/jobs/:id/:action with a
// confirm modal.

const REFRESH_MS = 5000;

const params = new URLSearchParams(location.search);
const token = params.get('token') || readCookie('slurmdash_token');
if (token) {
  document.cookie = `slurmdash_token=${token}; SameSite=Strict; path=/`;
  // Strip the token from the URL so it isn't pasted by accident.
  if (params.has('token')) {
    params.delete('token');
    const url = location.pathname + (params.toString() ? '?' + params.toString() : '');
    history.replaceState(null, '', url);
  }
}

function readCookie(name) {
  const m = document.cookie.match(new RegExp('(?:^|; )' + name + '=([^;]*)'));
  return m ? decodeURIComponent(m[1]) : null;
}

async function fetchJson(path, init = {}) {
  const opts = { credentials: 'include', ...init };
  opts.headers = { ...(opts.headers || {}), Authorization: `Bearer ${token}` };
  const r = await fetch(path, opts);
  if (r.status === 204) return null;
  if (!r.ok) throw new Error(`${r.status} ${r.statusText}`);
  return r.json();
}

// ---- Rendering helpers -----------------------------------------------

function gradeClass(pct) {
  if (pct >= 95) return 'critical';
  if (pct >= 80) return 'high';
  if (pct >= 50) return 'med';
  return 'low';
}

function barRow(label, pct, suffix) {
  const cls = gradeClass(pct);
  const w = Math.max(0, Math.min(100, pct));
  return `<div class="bar-row">
    <div class="label">${label}</div>
    <div class="bar-track"><div class="bar-fill ${cls}" style="width:${w}%"></div></div>
    <div class="suffix">${suffix}</div>
  </div>`;
}

function humanMb(mb) {
  if (mb >= 1024 * 1024) return (mb / 1024 / 1024).toFixed(1) + 'TB';
  if (mb >= 1024) return (mb / 1024).toFixed(1) + 'GB';
  return mb + 'MB';
}

function hms(secs) {
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const s = Math.floor(secs % 60);
  if (h > 0) return `${h}:${String(m).padStart(2,'0')}:${String(s).padStart(2,'0')}`;
  return `${String(m).padStart(2,'0')}:${String(s).padStart(2,'0')}`;
}

function aiotPct(a) {
  if (!a || a.total === 0) return 0;
  return (a.allocated / a.total) * 100;
}

function aiotPctU64(a) {
  if (!a || a.total === 0) return 0;
  return Number((BigInt(a.allocated) * 100n / (BigInt(a.total) || 1n)));
}

// ---- Panels -----------------------------------------------------------

function renderResources(r) {
  const root = document.getElementById('resources');
  const cpuPct = aiotPct(r.cpus);
  const gpuPct = aiotPct(r.gpus);
  const memPct = aiotPctU64(r.memory_mb);
  const hasGpu = r.gpus && r.gpus.total > 0;
  const parts = [];
  parts.push(barRow('CPU', cpuPct, `${r.cpus.allocated}/${r.cpus.total}`));
  if (hasGpu) parts.push(barRow('GPU', gpuPct, `${r.gpus.allocated}/${r.gpus.total}`));
  parts.push(barRow('MEM', memPct, `${humanMb(r.memory_mb.allocated)} / ${humanMb(r.memory_mb.total)}`));
  parts.push(`<div class="node-line">
    <span class="alloc">alloc ${r.nodes.allocated}</span>
    <span class="idle">idle ${r.nodes.idle}</span>
    <span class="other">other ${r.nodes.other}</span>
    <span>total ${r.nodes.total}</span>
  </div>`);
  root.innerHTML = parts.join('');
}

function renderQueue(jobs) {
  const root = document.getElementById('queue');
  const counts = {};
  for (const j of jobs) {
    const k = stateShort(j.state);
    counts[k] = (counts[k] || 0) + 1;
  }
  const max = Math.max(1, ...Object.values(counts));
  const priority = ['R','PD','CG','S','H'];
  const ordered = priority.filter(p => counts[p]);
  for (const k of Object.keys(counts).sort()) {
    if (!ordered.includes(k)) ordered.push(k);
  }
  root.innerHTML = ordered.map(k => {
    const w = (counts[k] / max) * 100;
    return `<div class="q-row">
      <span class="q-state ${k}">${k}</span>
      <span class="q-fill ${k}" style="width:${w}%"></span>
      <span class="count">${counts[k]}</span>
    </div>`;
  }).join('') || '<div class="muted">no jobs</div>';
}

function renderEnding(jobs) {
  const candidates = jobs
    .filter(j => stateShort(j.state) === 'R')
    .filter(j => j.elapsed_seconds != null && j.time_limit_seconds != null)
    .map(j => ({ j, remaining: Math.max(0, j.time_limit_seconds - j.elapsed_seconds) }))
    .sort((a, b) => a.remaining - b.remaining)
    .slice(0, 8);
  const root = document.getElementById('ending');
  if (!candidates.length) {
    root.innerHTML = '<div class="muted">no running jobs</div>';
    return;
  }
  root.innerHTML = candidates.map(({ j, remaining }) => {
    const cls = remaining < 300 ? 't-critical' : remaining < 1800 ? 't-medium' : 't-low';
    return `<div class="row-job">
      <span class="id">${j.job_id}</span>
      <span class="name">${escape(j.name)}</span>
      <span class="remaining ${cls}">-${hms(remaining)}</span>
    </div>`;
  }).join('');
}

function renderPartitions(parts) {
  const root = document.getElementById('partitions');
  if (!parts.length) {
    root.innerHTML = '<div class="muted">no partitions</div>';
    return;
  }
  root.innerHTML = parts.map(p => {
    const cpuPct = aiotPct(p.cpus);
    const memPct = p.memory_mb_per_node && p.nodes.total
      ? ((p.memory_mb_per_node * p.nodes.allocated) / (p.memory_mb_per_node * p.nodes.total)) * 100 : 0;
    const hasGpu = p.gpus_per_node && p.gpus_per_node > 0;
    const gpuPct = hasGpu && p.nodes.total
      ? ((p.gpus_per_node * p.nodes.allocated) / (p.gpus_per_node * p.nodes.total)) * 100 : 0;
    return `<div class="partition">
      <span class="name">${escape(p.name)}</span>
      ${seg('cpu', cpuPct)}
      ${hasGpu ? seg('gpu', gpuPct) : '<span></span>'}
      ${seg('mem', memPct)}
      <span class="nodes">${p.nodes.allocated}/${p.nodes.total} nodes</span>
    </div>`;
  }).join('');
}

function seg(label, pct) {
  const cls = gradeClass(pct);
  return `<div class="seg">
    <span class="seg-label">${label}</span>
    <div class="bar-track"><div class="bar-fill ${cls}" style="width:${Math.max(0,Math.min(100,pct))}%"></div></div>
    <span class="seg-pct">${Math.round(pct)}%</span>
  </div>`;
}

function renderJobs(jobs, readonly) {
  const tbody = document.querySelector('#jobs tbody');
  tbody.innerHTML = jobs.map(j => {
    const k = stateShort(j.state);
    const actions = readonly ? '' : `
      <button class="btn danger" data-job="${j.job_id}" data-action="cancel">cancel</button>
      <button class="btn warn"   data-job="${j.job_id}" data-action="hold">hold</button>`;
    return `<tr>
      <td>${j.job_id}</td>
      <td>${escape(j.partition)}</td>
      <td>${escape(j.name)}</td>
      <td>${escape(j.user)}</td>
      <td class="state ${k}">${k}</td>
      <td>${j.elapsed_seconds != null ? hms(j.elapsed_seconds) : '-'}</td>
      <td>${j.time_limit_seconds != null ? hms(j.time_limit_seconds) : '-'}</td>
      <td>${j.nodes}</td>
      <td>${escape(j.reason_or_nodelist)}</td>
      <td class="row-actions">${actions}</td>
    </tr>`;
  }).join('');
}

function stateShort(s) {
  if (typeof s === 'string') return s;
  return Object.keys(s)[0] ? s[Object.keys(s)[0]] || Object.keys(s)[0] : 'UNK';
}

function escape(s) {
  return String(s ?? '').replace(/[&<>"']/g, c => ({
    '&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'
  }[c]));
}

// ---- Modal + actions --------------------------------------------------

let pending = null;
function openModal(action, jobId) {
  pending = { action, jobId };
  document.getElementById('modal-title').textContent =
    `${action} job ${jobId}`;
  document.getElementById('modal-cmd').textContent =
    action === 'cancel' ? `scancel ${jobId}` : `scontrol ${action} ${jobId}`;
  document.getElementById('modal').hidden = false;
}
function closeModal() {
  pending = null;
  document.getElementById('modal').hidden = true;
}

document.addEventListener('click', async (e) => {
  const t = e.target;
  if (t.matches('button[data-action]')) {
    openModal(t.dataset.action, t.dataset.job);
  } else if (t.id === 'modal-cancel') {
    closeModal();
  } else if (t.id === 'modal-ok') {
    if (!pending) return;
    try {
      await fetchJson(`/api/jobs/${encodeURIComponent(pending.jobId)}/${pending.action}`, { method: 'POST' });
      closeModal();
      refresh();
    } catch (err) {
      alert(err.message);
    }
  }
});

document.addEventListener('keydown', (e) => {
  if (e.key === 'Escape') {
    closeModal();
    closeAssist();
  }
  if (e.key === 'k' && (e.metaKey || e.ctrlKey)) {
    e.preventDefault();
    openAssist();
  }
  // 'r' triggers refresh only when no text input is focused.
  if (
    e.key === 'r' &&
    !e.metaKey && !e.ctrlKey &&
    document.activeElement?.tagName !== 'INPUT'
  ) refresh();
});

// ---- Assist modal -----------------------------------------------------

function openAssist() {
  document.getElementById('assist').hidden = false;
  document.getElementById('assist-status').textContent = '';
  document.getElementById('assist-response').textContent = '';
  const input = document.getElementById('assist-input');
  input.value = '';
  input.focus();
}
function closeAssist() {
  document.getElementById('assist').hidden = true;
}

document.addEventListener('click', (e) => {
  if (e.target.id === 'assist-close') closeAssist();
  if (e.target.id === 'assist-send') sendAssist();
  if (e.target.matches('button[data-assist-cmd]')) {
    const action = e.target.dataset.assistAction;
    const job = e.target.dataset.assistJob;
    closeAssist();
    openModal(action, job);
  }
});
document.getElementById('assist-input').addEventListener('keydown', (e) => {
  if (e.key === 'Enter') {
    e.preventDefault();
    sendAssist();
  }
});

let selectedJobId = null; // Reserved for a future "selected row" indicator.

async function sendAssist() {
  const input = document.getElementById('assist-input');
  const status = document.getElementById('assist-status');
  const out = document.getElementById('assist-response');
  const prompt = input.value.trim();
  if (!prompt) return;
  status.textContent = 'thinking…';
  out.textContent = '';
  try {
    const body = JSON.stringify({ prompt, job_id: selectedJobId });
    const r = await fetchJson('/api/assist', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body,
    });
    status.textContent = '';
    renderAssistResponse(r);
  } catch (err) {
    status.textContent = '';
    out.textContent = `error: ${err.message}`;
  }
}

function renderAssistResponse(r) {
  const out = document.getElementById('assist-response');
  const head = `<div class="meta">[${escape(r.provider)} · ${escape(r.model)}]</div>`;
  const body = `<div>${escape(r.text)}</div>`;
  let cmds = '';
  if (r.commands && r.commands.length) {
    cmds = '<hr style="border-color:var(--border);margin:8px 0">';
    for (const c of r.commands) {
      const kind = c.kind?.type ?? '';
      const job = c.kind?.job_id ?? '';
      let action = '';
      if (kind === 'Cancel') action = 'cancel';
      else if (kind === 'Hold') action = 'hold';
      else if (kind === 'Release') action = 'release';
      else if (kind === 'Requeue') action = 'requeue';
      const btn = (action && job)
        ? `<button class="btn warn" data-assist-cmd data-assist-action="${action}" data-assist-job="${escape(job)}">confirm</button>`
        : `<span class="muted">manual</span>`;
      cmds += `<div class="cmd-row"><code>${escape(c.preview)}</code>${btn}</div>`;
    }
  }
  out.innerHTML = head + body + cmds;
}

// ---- Main loop --------------------------------------------------------

document.getElementById('refresh-seconds').textContent = String(REFRESH_MS / 1000);

async function refresh() {
  try {
    const data = await fetchJson('/api/dashboard');
    document.getElementById('cluster').textContent = data.cluster.name;
    document.getElementById('readonly').hidden = !data.readonly;
    const snap = data.snapshot;
    document.getElementById('status').textContent = snap.last_error
      ? `error: ${snap.last_error}`
      : (snap.last_refresh ? `updated ${new Date(snap.last_refresh).toLocaleTimeString()}` : 'no data');
    renderResources(snap.resources);
    renderQueue(snap.jobs);
    renderEnding(snap.jobs);
    renderPartitions(snap.partitions);
    renderJobs(snap.jobs, data.readonly);
  } catch (err) {
    document.getElementById('status').textContent = `error: ${err.message}`;
  }
}

refresh();
setInterval(refresh, REFRESH_MS);
