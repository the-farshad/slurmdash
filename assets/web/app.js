// slurmdash web — richer dashboard with KPI strip, sparklines,
// state donut, wait histogram, and a filterable / sortable jobs
// table. Vanilla JS + inline SVG; no external dependencies.

const REFRESH_MS = 5000;

const params = new URLSearchParams(location.search);
const token = params.get('token') || readCookie('slurmdash_token');
if (token) {
  document.cookie = `slurmdash_token=${token}; SameSite=Strict; path=/`;
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

// ---- Persistent state ------------------------------------------------

const ui = {
  sortKey: 'state',
  sortDesc: false,
  filterText: '',
  jobs: [],     // last fetched
  readonly: false,
};

// ---- Small helpers ---------------------------------------------------

function gradeClass(pct) {
  if (pct >= 95) return 'critical';
  if (pct >= 80) return 'high';
  if (pct >= 50) return 'med';
  return 'low';
}

function gradeColor(pct) {
  if (pct >= 95) return 'var(--critical)';
  if (pct >= 80) return 'var(--high)';
  if (pct >= 50) return 'var(--med)';
  return 'var(--low)';
}

function humanMb(mb) {
  if (mb == null) return '—';
  if (mb >= 1024 * 1024) return (mb / 1024 / 1024).toFixed(1) + 'TB';
  if (mb >= 1024) return (mb / 1024).toFixed(1) + 'GB';
  return mb + 'MB';
}

function shortDur(s) {
  if (s == null) return '—';
  if (s < 60) return `${s}s`;
  if (s < 3600) return `${Math.floor(s/60)}m`;
  if (s < 86400) return `${Math.floor(s/3600)}h`;
  return `${Math.floor(s/86400)}d`;
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

// Rust's JobState enum serializes unit variants as their full name
// ("Running", "Pending", "Held", …) and Other(s) as `{Other: "FOO"}`.
// The TUI / CSS / chart palettes use the Slurm short code (R, PD, …),
// so we normalize everything to short codes here.
const STATE_TO_SHORT = {
  Pending: 'PD', Running: 'R', Suspended: 'S', Completing: 'CG',
  Completed: 'CD', Cancelled: 'CA', Failed: 'F', Timeout: 'TO',
  NodeFail: 'NF', Preempted: 'PR', BootFail: 'BF', Deadline: 'DL',
  OutOfMemory: 'OOM', Held: 'H',
};
const STATE_LONG_LABEL = {
  R: 'Running', PD: 'Pending', S: 'Suspended', CG: 'Completing',
  CD: 'Completed', CA: 'Cancelled', F: 'Failed', TO: 'Timeout',
  NF: 'Node Fail', PR: 'Preempted', BF: 'Boot Fail', DL: 'Deadline',
  OOM: 'Out of Memory', H: 'Held',
};

function stateShort(s) {
  if (typeof s === 'string') return STATE_TO_SHORT[s] || s;
  // Other("FOO") arrives as { Other: "FOO" } — return the inner string
  // as the short code; it likely isn't in our palette and will fall
  // back to muted, which is the correct behavior for unknown states.
  const k = Object.keys(s)[0];
  return k ? (s[k] || k) : 'UNK';
}

function escape(s) {
  return String(s ?? '').replace(/[&<>"']/g, c => ({
    '&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'
  }[c]));
}

// Parse a GRES string like "gpu:a100:2" → 2; "gpu:4" → 4; "" → 0.
function gpuCount(gres) {
  if (!gres) return 0;
  let total = 0;
  for (const t of gres.split(',')) {
    const tok = t.trim();
    if (!tok.startsWith('gpu')) continue;
    const parts = tok.split(':');
    let n;
    if (parts.length === 1) n = 1;
    else if (parts.length === 2) n = parseInt(parts[1], 10);
    else n = parseInt((parts[2] || '').split('(')[0], 10);
    if (Number.isFinite(n)) total += n;
  }
  return total;
}

// Wait time helper: matches Job::wait_seconds() in slurm/model.rs.
function waitSeconds(j) {
  if (!j.submit_time) return null;
  const submit = new Date(j.submit_time).getTime() / 1000;
  const st = stateShort(j.state);
  if (st === 'R') {
    if (!j.start_time) return null;
    const start = new Date(j.start_time).getTime() / 1000;
    return Math.max(0, start - submit);
  }
  if (st === 'PD') {
    return Math.max(0, Date.now() / 1000 - submit);
  }
  return null;
}

// ---- KPI strip -------------------------------------------------------

function renderKpis(snap) {
  const jobs = snap.jobs;
  let running = 0, pending = 0, held = 0, failed = 0;
  let waitSum = 0, waitN = 0, gpuJobs = 0;
  for (const j of jobs) {
    const st = stateShort(j.state);
    if (st === 'R') running++;
    else if (st === 'PD') pending++;
    else if (st === 'H') held++;
    else if (st === 'F' || st === 'TO' || st === 'NF' || st === 'BF' || st === 'DL' || st === 'OOM') failed++;
    const g = gpuCount(j.gres);
    if (g > 0) gpuJobs++;
    const w = waitSeconds(j);
    if (w != null) { waitSum += w; waitN++; }
  }
  const r = snap.resources;
  document.getElementById('kpi-jobs').textContent = jobs.length;
  document.getElementById('kpi-running').textContent = running;
  document.getElementById('kpi-pending').textContent = pending;
  document.getElementById('kpi-held').textContent = held;
  document.getElementById('kpi-failed').textContent = failed;
  document.getElementById('kpi-nodes').textContent = `${r.nodes.allocated}/${r.nodes.total}`;
  document.getElementById('kpi-gpus').textContent = r.gpus?.allocated ?? 0;
  document.getElementById('kpi-gpu-jobs').textContent = gpuJobs;
  document.getElementById('kpi-wait').textContent = waitN > 0 ? shortDur(Math.round(waitSum / waitN)) : '—';
}

// ---- Sparkline trend charts -----------------------------------------

function sparkline(values, color, opts = {}) {
  const w = opts.width || 200;
  const h = opts.height || 56;
  const max = opts.max ?? 100;
  if (!values || values.length < 2) {
    return `<svg viewBox="0 0 ${w} ${h}" preserveAspectRatio="none">
      <text x="${w/2}" y="${h/2}" text-anchor="middle" fill="var(--muted)" font-size="10">collecting…</text>
    </svg>`;
  }
  const n = values.length;
  const stepX = w / Math.max(1, n - 1);
  const m = Math.max(max, 1);
  const pts = values.map((v, i) => `${(i*stepX).toFixed(1)},${(h - (Math.max(0, Math.min(m, v))/m)*h).toFixed(1)}`).join(' ');
  // Area fill under the line.
  const areaPts = `0,${h} ${pts} ${w},${h}`;
  return `<svg viewBox="0 0 ${w} ${h}" preserveAspectRatio="none">
    <polygon points="${areaPts}" fill="${color}" fill-opacity="0.15"/>
    <polyline points="${pts}" fill="none" stroke="${color}" stroke-width="1.5" stroke-linejoin="round"/>
  </svg>`;
}

function stackedSparkline(seriesA, seriesB, colorA, colorB, opts = {}) {
  // Stacked area: seriesA on bottom, seriesA+seriesB on top. Used for
  // queue depth (running stacked under pending).
  const w = opts.width || 200;
  const h = opts.height || 56;
  if (!seriesA || seriesA.length < 2) {
    return `<svg viewBox="0 0 ${w} ${h}" preserveAspectRatio="none">
      <text x="${w/2}" y="${h/2}" text-anchor="middle" fill="var(--muted)" font-size="10">collecting…</text>
    </svg>`;
  }
  const n = seriesA.length;
  let m = 1;
  for (let i = 0; i < n; i++) m = Math.max(m, seriesA[i] + (seriesB[i] || 0));
  const stepX = w / Math.max(1, n - 1);
  const ya = seriesA.map((v, i) => `${(i*stepX).toFixed(1)},${(h - (v/m)*h).toFixed(1)}`);
  const yb = seriesA.map((v, i) => `${(i*stepX).toFixed(1)},${(h - ((v + (seriesB[i]||0))/m)*h).toFixed(1)}`);
  const bottom = `0,${h} ${ya.join(' ')} ${w},${h}`;
  const between = `${ya.join(' ')} ${[...yb].reverse().join(' ')}`;
  return `<svg viewBox="0 0 ${w} ${h}" preserveAspectRatio="none">
    <polygon points="${bottom}" fill="${colorA}" fill-opacity="0.45"/>
    <polygon points="${between}" fill="${colorB}" fill-opacity="0.35"/>
    <polyline points="${yb.join(' ')}" fill="none" stroke="${colorB}" stroke-width="1.2"/>
    <polyline points="${ya.join(' ')}" fill="none" stroke="${colorA}" stroke-width="1.5"/>
  </svg>`;
}

function renderTrends(history) {
  const pts = history?.points || [];
  const cpu = pts.map(p => p.cpu_pct);
  const gpu = pts.map(p => p.gpu_pct);
  const mem = pts.map(p => p.mem_pct);
  const run = pts.map(p => p.n_running);
  const pend = pts.map(p => p.n_pending);

  const last = pts.length ? pts[pts.length - 1] : null;
  document.getElementById('trend-cpu-now').textContent = last ? Math.round(last.cpu_pct) + '%' : '—';
  document.getElementById('trend-gpu-now').textContent = last ? Math.round(last.gpu_pct) + '%' : '—';
  document.getElementById('trend-mem-now').textContent = last ? Math.round(last.mem_pct) + '%' : '—';
  document.getElementById('trend-queue-now').textContent = last ? `${last.n_running}R / ${last.n_pending}PD` : '—';

  const colCpu = last ? gradeColor(last.cpu_pct) : 'var(--low)';
  const colGpu = last ? gradeColor(last.gpu_pct) : 'var(--low)';
  const colMem = last ? gradeColor(last.mem_pct) : 'var(--low)';
  document.getElementById('trend-cpu').innerHTML = sparkline(cpu, colCpu);
  document.getElementById('trend-gpu').innerHTML = sparkline(gpu, colGpu);
  document.getElementById('trend-mem').innerHTML = sparkline(mem, colMem);
  document.getElementById('trend-queue').innerHTML = stackedSparkline(run, pend, 'var(--running)', 'var(--pending)');
}

// ---- State donut ----------------------------------------------------

const STATE_COLORS = {
  R:  'var(--chart-running)',
  PD: 'var(--chart-pending)',
  CG: 'var(--chart-completing)',
  CD: 'var(--chart-completed)',
  F:  'var(--chart-failed)',
  TO: 'var(--chart-failed)',
  NF: 'var(--chart-failed)',
  BF: 'var(--chart-failed)',
  DL: 'var(--chart-failed)',
  OOM:'var(--chart-failed)',
  CA: 'var(--chart-cancelled)',
  H:  'var(--chart-held)',
  S:  'var(--chart-suspended)',
};

function renderStateDonut(jobs) {
  const counts = {};
  for (const j of jobs) {
    const k = stateShort(j.state);
    counts[k] = (counts[k] || 0) + 1;
  }
  const segs = Object.entries(counts)
    .sort((a, b) => b[1] - a[1])
    .map(([k, v]) => ({ key: k, value: v, color: STATE_COLORS[k] || 'var(--muted)' }));

  const total = segs.reduce((a, s) => a + s.value, 0);
  const W = 160;
  const R = 60;
  const C = 2 * Math.PI * R;
  const cx = W/2, cy = W/2;

  let cumulative = 0;
  const arcs = segs.map(s => {
    const len = (s.value / Math.max(1, total)) * C;
    const offset = C * 0.25 - cumulative;  // start at top
    cumulative += len;
    return `<circle cx="${cx}" cy="${cy}" r="${R}" fill="none"
      stroke="${s.color}" stroke-width="22"
      stroke-dasharray="${len.toFixed(2)} ${(C - len).toFixed(2)}"
      stroke-dashoffset="${offset.toFixed(2)}"/>`;
  }).join('');

  document.getElementById('state-donut').innerHTML = `
    <svg width="${W}" height="${W}" viewBox="0 0 ${W} ${W}">
      ${arcs}
      <text x="${cx}" y="${cy - 4}" text-anchor="middle" fill="var(--fg)" font-size="20" font-weight="700">${total}</text>
      <text x="${cx}" y="${cy + 14}" text-anchor="middle" fill="var(--muted)" font-size="10">jobs</text>
    </svg>`;

  document.getElementById('state-legend').innerHTML = segs.map(s => {
    const label = STATE_LONG_LABEL[s.key] || s.key;
    return `<div><span class="sw" style="background:${s.color}"></span>${label}<span class="cnt">${s.value}</span></div>`;
  }).join('') || '<div class="muted">no jobs</div>';
}

// ---- Wait histogram -------------------------------------------------

const WAIT_BUCKETS = [
  { label: '< 1m',    cls: 'b0', max:    60 },
  { label: '1–5m',    cls: 'b1', max:   300 },
  { label: '5–30m',   cls: 'b2', max:  1800 },
  { label: '30m–2h',  cls: 'b3', max:  7200 },
  { label: '2–12h',   cls: 'b4', max: 43200 },
  { label: '> 12h',   cls: 'b5', max: Infinity },
];

function renderWaitHist(jobs) {
  const counts = WAIT_BUCKETS.map(() => 0);
  for (const j of jobs) {
    const w = waitSeconds(j);
    if (w == null) continue;
    for (let i = 0; i < WAIT_BUCKETS.length; i++) {
      if (w < WAIT_BUCKETS[i].max) { counts[i]++; break; }
    }
  }
  const max = Math.max(1, ...counts);
  document.getElementById('wait-hist').innerHTML = WAIT_BUCKETS.map((b, i) => {
    const pct = (counts[i] / max) * 100;
    return `<div class="hist-row">
      <div class="label">${b.label}</div>
      <div class="hist-bar"><div class="hist-fill ${b.cls}" style="width:${pct}%"></div></div>
      <div class="count">${counts[i]}</div>
    </div>`;
  }).join('');
}

// ---- Top users ------------------------------------------------------

function renderTopUsers(jobs) {
  const byUser = {};
  for (const j of jobs) {
    if (!byUser[j.user]) byUser[j.user] = { total: 0, R: 0, PD: 0, F: 0 };
    byUser[j.user].total++;
    const st = stateShort(j.state);
    if (st === 'R') byUser[j.user].R++;
    else if (st === 'PD') byUser[j.user].PD++;
    else if (st === 'F' || st === 'TO' || st === 'NF') byUser[j.user].F++;
  }
  const entries = Object.entries(byUser).sort((a, b) => b[1].total - a[1].total).slice(0, 10);
  const max = Math.max(1, ...entries.map(e => e[1].total));
  const root = document.getElementById('top-users');
  if (!entries.length) {
    root.innerHTML = '<div class="muted">no jobs</div>';
    return;
  }
  root.innerHTML = entries.map(([user, s]) => {
    const pct = (s.total / max) * 100;
    return `<div class="tu-row">
      <div class="label">${escape(user)}</div>
      <div class="tu-bar"><div class="tu-fill" style="width:${pct}%"></div></div>
      <div class="count">${s.total}</div>
    </div>`;
  }).join('');
}

// ---- Resources + ending soon (existing) -----------------------------

function barRow(label, pct, suffix) {
  const cls = gradeClass(pct);
  const w = Math.max(0, Math.min(100, pct));
  return `<div class="bar-row">
    <div class="label">${label}</div>
    <div class="bar-track"><div class="bar-fill ${cls}" style="width:${w}%"></div></div>
    <div class="suffix">${suffix}</div>
  </div>`;
}

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

// ---- Partitions -----------------------------------------------------

function seg(label, pct) {
  const cls = gradeClass(pct);
  return `<div class="seg">
    <span class="seg-label">${label}</span>
    <div class="bar-track"><div class="bar-fill ${cls}" style="width:${Math.max(0,Math.min(100,pct))}%"></div></div>
    <span class="seg-pct">${Math.round(pct)}%</span>
  </div>`;
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

// ---- Filter (mirrors the TUI's parse_filter) ------------------------

function parseFilter(s) {
  const p = {
    free: [], user: [], partition: [], state: [], name: [], jobId: [], reason: [],
    gpuOnly: false, cpuOnly: false, memMin: null, memMax: null,
  };
  for (const tok of s.split(/\s+/).filter(Boolean)) {
    const low = tok.toLowerCase();
    if (low === 'gpu') { p.gpuOnly = true; continue; }
    if (low === 'cpu') { p.cpuOnly = true; continue; }
    const i = tok.indexOf(':');
    if (i > 0) {
      const field = tok.slice(0, i).toLowerCase();
      const value = tok.slice(i + 1);
      if (!value) continue;
      switch (field) {
        case 'user': case 'u': p.user.push(value); continue;
        case 'partition': case 'part': case 'p': p.partition.push(value); continue;
        case 'state': case 'st': case 's': p.state.push(value.toUpperCase()); continue;
        case 'name': case 'n': p.name.push(value); continue;
        case 'id': case 'jobid': case 'job': p.jobId.push(value); continue;
        case 'reason': case 'r': p.reason.push(value); continue;
        case 'gpu':
          if (['none', 'no', '0'].includes(value.toLowerCase())) p.cpuOnly = true;
          else p.gpuOnly = true;
          continue;
        case 'mem': case 'm': {
          let cmp = '>', num = value;
          if (value.startsWith('>=')) { cmp = '>'; num = value.slice(2); }
          else if (value.startsWith('<=')) { cmp = '<'; num = value.slice(2); }
          else if (value.startsWith('>'))  { cmp = '>'; num = value.slice(1); }
          else if (value.startsWith('<'))  { cmp = '<'; num = value.slice(1); }
          else if (value.startsWith('='))  { cmp = '='; num = value.slice(1); }
          const mb = parseMem(num);
          if (mb != null) {
            if (cmp === '>') p.memMin = mb;
            else if (cmp === '<') p.memMax = mb;
            else { p.memMin = mb; p.memMax = mb; }
          }
          continue;
        }
        default: p.free.push(tok);
      }
    } else {
      p.free.push(tok);
    }
  }
  return p;
}

function parseMem(raw) {
  const m = raw.match(/^([\d.]+)\s*([kmgtKMGT]?)[nc]?$/);
  if (!m) return null;
  const num = parseFloat(m[1]);
  if (!Number.isFinite(num)) return null;
  const u = m[2].toUpperCase();
  switch (u) {
    case '': case 'M': return Math.round(num);
    case 'K': return Math.round(num / 1024);
    case 'G': return Math.round(num * 1024);
    case 'T': return Math.round(num * 1024 * 1024);
    default: return null;
  }
}

function matchesFilter(j, p) {
  const st = stateShort(j.state);
  const gpus = gpuCount(j.gres);
  if (p.gpuOnly && gpus === 0) return false;
  if (p.cpuOnly && gpus > 0) return false;
  if (p.memMin != null && (j.min_mem_mb == null || j.min_mem_mb < p.memMin)) return false;
  if (p.memMax != null && (j.min_mem_mb == null || j.min_mem_mb > p.memMax)) return false;
  const lc = s => String(s ?? '').toLowerCase();
  for (const q of p.free) {
    const ql = q.toLowerCase();
    if (!(lc(j.job_id).includes(ql) || lc(j.name).includes(ql) ||
          lc(j.user).includes(ql) || lc(j.partition).includes(ql) ||
          lc(j.reason_or_nodelist).includes(ql))) return false;
  }
  for (const q of p.user) if (!lc(j.user).includes(q.toLowerCase())) return false;
  for (const q of p.partition) if (!lc(j.partition).includes(q.toLowerCase())) return false;
  for (const q of p.name) if (!lc(j.name).includes(q.toLowerCase())) return false;
  for (const q of p.jobId) if (!lc(j.job_id).includes(q.toLowerCase())) return false;
  for (const q of p.reason) if (!lc(j.reason_or_nodelist).includes(q.toLowerCase())) return false;
  for (const q of p.state) {
    if (st.toUpperCase() !== q && !String(j.state).toUpperCase().includes(q)) return false;
  }
  return true;
}

// ---- Jobs table -----------------------------------------------------

function renderJobsTotals(jobs, totalUnfiltered) {
  const root = document.getElementById('jobs-totals');
  if (!root) return;
  // Always show the count + filter chip so a zero-result filter still
  // produces feedback.
  const parts = [];
  parts.push(`<span class="label">Σ</span> <span class="value accent">${jobs.length} job${jobs.length === 1 ? '' : 's'}</span>`);
  if (ui.filterText.trim().length > 0) {
    parts.push('<span class="sep">·</span>');
    parts.push(`<span class="label">filter</span> <span class="value accent">/${escape(ui.filterText.trim())}/</span> <span class="label">(${jobs.length} of ${totalUnfiltered})</span>`);
  }
  if (jobs.length === 0) {
    root.innerHTML = parts.join(' ');
    return;
  }
  // Aggregates over the filtered set.
  const byState = {};
  let nodes = 0, gpus = 0;
  let memSum = 0, memN = 0;
  let waitSum = 0, waitN = 0;
  let limitMax = 0, elapsedTotal = 0;
  for (const j of jobs) {
    const k = stateShort(j.state);
    byState[k] = (byState[k] || 0) + 1;
    nodes += j.nodes || 0;
    gpus += gpuCount(j.gres);
    if (j.min_mem_mb != null) { memSum += j.min_mem_mb; memN++; }
    const w = waitSeconds(j);
    if (w != null) { waitSum += w; waitN++; }
    if (j.time_limit_seconds) limitMax = Math.max(limitMax, j.time_limit_seconds);
    if (j.elapsed_seconds) elapsedTotal += j.elapsed_seconds;
  }
  // State chips.
  const chips = Object.entries(byState)
    .sort((a, b) => b[1] - a[1])
    .map(([k, n]) => `<span class="chip ${k}">${n}${k}</span>`)
    .join(' ');
  if (chips) parts.push('<span class="sep">·</span>', chips);
  parts.push('<span class="sep">·</span>', `<span class="label">Σnodes</span> <span class="value">${nodes}</span>`);
  if (gpus > 0) parts.push('<span class="sep">·</span>', `<span class="label">ΣGPUs</span> <span class="value green">${gpus}</span>`);
  if (memN > 0) parts.push('<span class="sep">·</span>', `<span class="label">Σmem</span> <span class="value">${humanMb(memSum)}</span>`);
  if (elapsedTotal > 0) parts.push('<span class="sep">·</span>', `<span class="label">Σelapsed</span> <span class="value green">${shortDur(elapsedTotal)}</span>`);
  if (limitMax > 0) parts.push('<span class="sep">·</span>', `<span class="label">max limit</span> <span class="value">${shortDur(limitMax)}</span>`);
  if (waitN > 0) parts.push('<span class="sep">·</span>', `<span class="label">avg wait</span> <span class="value warn">${shortDur(Math.round(waitSum / waitN))}</span>`);
  root.innerHTML = parts.join(' ');
}

function compare(a, b, key) {
  switch (key) {
    case 'job_id': {
      const ai = parseInt(a.job_id, 10), bi = parseInt(b.job_id, 10);
      if (Number.isFinite(ai) && Number.isFinite(bi)) return ai - bi;
      return String(a.job_id).localeCompare(String(b.job_id));
    }
    case 'partition': return a.partition.localeCompare(b.partition);
    case 'name':      return a.name.localeCompare(b.name);
    case 'user':      return a.user.localeCompare(b.user);
    case 'state':     return stateShort(a.state).localeCompare(stateShort(b.state));
    case 'elapsed_seconds': return (a.elapsed_seconds || 0) - (b.elapsed_seconds || 0);
    case 'time_limit_seconds': return (a.time_limit_seconds || 0) - (b.time_limit_seconds || 0);
    case 'wait':      return (waitSeconds(a) || 0) - (waitSeconds(b) || 0);
    case 'gpus':      return gpuCount(a.gres) - gpuCount(b.gres);
    case 'min_mem_mb':return (a.min_mem_mb || 0) - (b.min_mem_mb || 0);
    case 'nodes':     return (a.nodes || 0) - (b.nodes || 0);
    default: return 0;
  }
}

function renderJobs() {
  const p = parseFilter(ui.filterText);
  const filtered = ui.jobs.filter(j => matchesFilter(j, p));
  filtered.sort((a, b) => {
    const c = compare(a, b, ui.sortKey);
    return ui.sortDesc ? -c : c;
  });
  document.getElementById('jobs-count').textContent =
    filtered.length === ui.jobs.length
      ? `${ui.jobs.length} jobs`
      : `${filtered.length} of ${ui.jobs.length} jobs`;
  renderJobsTotals(filtered, ui.jobs.length);

  const tbody = document.querySelector('#jobs tbody');
  if (!filtered.length) {
    tbody.innerHTML = `<tr><td colspan="13" class="muted" style="text-align:center;padding:24px">no matching jobs</td></tr>`;
    return;
  }
  tbody.innerHTML = filtered.map(j => {
    const k = stateShort(j.state);
    const g = gpuCount(j.gres);
    const w = waitSeconds(j);
    const actions = ui.readonly ? '' : `
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
      <td>${w != null ? shortDur(Math.round(w)) : '—'}</td>
      <td class="gpus">${g > 0 ? g : '—'}</td>
      <td class="mem">${humanMb(j.min_mem_mb)}</td>
      <td class="nodes-col">${j.nodes}</td>
      <td>${escape(j.reason_or_nodelist)}</td>
      <td class="row-actions">${actions}</td>
    </tr>`;
  }).join('');

  // Sort indicator on headers.
  document.querySelectorAll('#jobs thead th').forEach(th => {
    th.classList.remove('sorted', 'desc');
    if (th.dataset.sort === ui.sortKey) {
      th.classList.add('sorted');
      if (ui.sortDesc) th.classList.add('desc');
    }
  });
}

// ---- Modal + actions ------------------------------------------------

let pending = null;
function openModal(action, jobId) {
  pending = { action, jobId };
  document.getElementById('modal-title').textContent = `${action} job ${jobId}`;
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
  if (t.matches('#jobs thead th[data-sort]')) {
    const key = t.dataset.sort;
    if (ui.sortKey === key) ui.sortDesc = !ui.sortDesc;
    else { ui.sortKey = key; ui.sortDesc = false; }
    renderJobs();
  } else if (t.matches('button[data-action]')) {
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
  } else if (t.id === 'refresh-btn') {
    refresh();
  }
});

document.getElementById('filter-input').addEventListener('input', (e) => {
  ui.filterText = e.target.value;
  renderJobs();
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
  if (
    e.key === 'r' && !e.metaKey && !e.ctrlKey &&
    document.activeElement?.tagName !== 'INPUT'
  ) refresh();
  // `Ctrl+/` focuses the filter input. `/` alone is reserved by
  // Firefox for "Search for text when you start typing" and our
  // preventDefault doesn't always beat the browser to it.
  if (
    e.key === '/' && (e.metaKey || e.ctrlKey) &&
    document.activeElement?.tagName !== 'INPUT'
  ) {
    e.preventDefault();
    document.getElementById('filter-input').focus();
  }
});

// ---- Assist modal ---------------------------------------------------

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
  if (e.key === 'Enter') { e.preventDefault(); sendAssist(); return; }
  // 1-9 confirm a proposed command — only when the input is empty so
  // we don't swallow digits the user actually wants to type.
  if (/^[1-9]$/.test(e.key) && e.target.value.length === 0) {
    const idx = parseInt(e.key, 10) - 1;
    const cmd = lastCommands[idx];
    if (cmd && cmd.action && cmd.job) {
      e.preventDefault();
      closeAssist();
      openModal(cmd.action, cmd.job);
    }
  }
});

let selectedJobId = null;
let lastCommands = [];
let thinkTicker = null;
let inFlight = false;

function setAssistThinking(on) {
  const status = document.getElementById('assist-status');
  const sendBtn = document.getElementById('assist-send');
  if (thinkTicker) { clearInterval(thinkTicker); thinkTicker = null; }
  if (!on) {
    status.innerHTML = '';
    sendBtn.disabled = false;
    return;
  }
  sendBtn.disabled = true;
  const frames = ['⠋','⠙','⠹','⠸','⠼','⠴','⠦','⠧','⠇','⠏'];
  let i = 0;
  const t0 = Date.now();
  const tick = () => {
    const elapsed = Math.floor((Date.now() - t0) / 1000);
    status.innerHTML =
      `<span class="spinner">${frames[i % frames.length]}</span>` +
      ` thinking… <span class="muted">${elapsed}s</span>`;
    i++;
  };
  tick();
  thinkTicker = setInterval(tick, 100);
}

async function sendAssist() {
  if (inFlight) return; // ignore double-Enter while waiting
  const input = document.getElementById('assist-input');
  const out = document.getElementById('assist-response');
  const prompt = input.value.trim();
  if (!prompt) return;
  inFlight = true;
  setAssistThinking(true);
  out.innerHTML = '';
  lastCommands = [];
  try {
    const body = JSON.stringify({ prompt, job_id: selectedJobId });
    const r = await fetchJson('/api/assist', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body,
    });
    setAssistThinking(false);
    renderAssistResponse(r);
    // Clear the input so the user can start typing a follow-up
    // immediately — without this they have to delete the previous
    // prompt every time, which is why Enter felt broken on the second
    // round-trip.
    input.value = '';
    input.focus();
  } catch (err) {
    setAssistThinking(false);
    out.innerHTML = `<div class="md" style="color:var(--failed)">error: ${escape(err.message)}</div>`;
  } finally {
    inFlight = false;
  }
}

// Minimal Markdown subset → HTML. We escape first, then transform the
// escaped string — the inserted tags are the only raw HTML in the
// output, so user-/model-supplied content stays safe from injection.
function renderMarkdown(text) {
  let s = escape(text ?? '');
  // Fenced code blocks (triple backticks). Optional language hint
  // after the opening fence is preserved as a class.
  s = s.replace(/```([a-zA-Z0-9_+-]*)\n?([\s\S]*?)```/g, (_m, lang, code) => {
    const cls = lang ? ` class="lang-${lang}"` : '';
    return `<pre><code${cls}>${code.replace(/\n$/, '')}</code></pre>`;
  });
  // Inline code.
  s = s.replace(/`([^`\n]+)`/g, '<code>$1</code>');
  // Headings (most → least specific).
  s = s.replace(/^### (.+)$/gm, '<h5>$1</h5>');
  s = s.replace(/^## (.+)$/gm, '<h4>$1</h4>');
  s = s.replace(/^# (.+)$/gm,  '<h3>$1</h3>');
  // Bold + italic. Ordered: bold first.
  s = s.replace(/\*\*([^*\n]+)\*\*/g, '<strong>$1</strong>');
  s = s.replace(/(?<!\*)\*([^*\n]+)\*(?!\*)/g, '<em>$1</em>');
  // Lists (one-level, contiguous lines starting with "- " or "* ").
  s = s.replace(/(^|\n)((?:- |\* )[^\n]+(?:\n(?:- |\* )[^\n]+)*)/g, (_m, lead, block) => {
    const items = block.split(/\n/).map(l => `<li>${l.replace(/^(?:- |\* )/, '')}</li>`).join('');
    return `${lead}<ul>${items}</ul>`;
  });
  // Numbered lists.
  s = s.replace(/(^|\n)((?:\d+\. )[^\n]+(?:\n\d+\. [^\n]+)*)/g, (_m, lead, block) => {
    const items = block.split(/\n/).map(l => `<li>${l.replace(/^\d+\.\s+/, '')}</li>`).join('');
    return `${lead}<ol>${items}</ol>`;
  });
  // Links: [text](url) — restrict the URL to safe schemes.
  s = s.replace(/\[([^\]]+)\]\((https?:\/\/[^)\s]+|mailto:[^)\s]+)\)/g,
                '<a href="$2" target="_blank" rel="noreferrer">$1</a>');
  // Horizontal rule.
  s = s.replace(/^---+$/gm, '<hr>');
  // Paragraph breaks — blank line → end of paragraph; single \n inside
  // a paragraph becomes <br>.
  const blocks = s.split(/\n{2,}/).map(b => {
    if (/^<(h[3-5]|ul|ol|pre|hr|blockquote)/i.test(b.trim())) return b;
    return `<p>${b.replace(/\n/g, '<br>')}</p>`;
  });
  return blocks.join('');
}

function renderAssistResponse(r) {
  const out = document.getElementById('assist-response');
  const head = `<div class="meta">[${escape(r.provider)} · ${escape(r.model)}]</div>`;
  const body = `<div class="md">${renderMarkdown(r.text)}</div>`;
  let cmds = '';
  lastCommands = [];
  if (r.commands && r.commands.length) {
    cmds = '<hr>';
    r.commands.slice(0, 9).forEach((c, idx) => {
      const num = idx + 1;
      const kind = c.kind?.type ?? '';
      const job = c.kind?.job_id ?? '';
      let action = '';
      if (kind === 'Cancel') action = 'cancel';
      else if (kind === 'Hold') action = 'hold';
      else if (kind === 'Release') action = 'release';
      else if (kind === 'Requeue') action = 'requeue';
      lastCommands.push({ action, job });
      const btn = (action && job)
        ? `<button class="btn warn" data-assist-cmd data-assist-action="${action}" data-assist-job="${escape(job)}">${num} · confirm</button>`
        : `<span class="muted">${num} · manual</span>`;
      cmds += `<div class="cmd-row"><span class="cmd-num">${num}.</span><code>${escape(c.preview)}</code>${btn}</div>`;
    });
  }
  out.innerHTML = head + body + cmds;
}

// ---- Main loop ------------------------------------------------------

document.getElementById('refresh-seconds').textContent = String(REFRESH_MS / 1000);

async function refresh() {
  try {
    const [dash, history] = await Promise.all([
      fetchJson('/api/dashboard'),
      fetchJson('/api/history').catch(() => ({ points: [] })),
    ]);
    document.getElementById('cluster').textContent = dash.cluster.name;
    document.getElementById('readonly').hidden = !dash.readonly;
    ui.readonly = dash.readonly;
    const snap = dash.snapshot;
    ui.jobs = snap.jobs;
    document.getElementById('status').textContent = snap.last_error
      ? `error: ${snap.last_error}`
      : (snap.last_refresh ? `updated ${new Date(snap.last_refresh).toLocaleTimeString()}` : 'no data');

    renderKpis(snap);
    renderTrends(history);
    renderStateDonut(snap.jobs);
    renderWaitHist(snap.jobs);
    renderTopUsers(snap.jobs);
    renderResources(snap.resources);
    renderEnding(snap.jobs);
    renderPartitions(snap.partitions);
    renderJobs();
  } catch (err) {
    document.getElementById('status').textContent = `error: ${err.message}`;
  }
}

refresh();
setInterval(refresh, REFRESH_MS);
