// Kleines Fenster beim Klick auf das Erweiterungs-Symbol: zeigt die Downloads aus Speedload.
const api = globalThis.browser ?? chrome;
const SERVER = 'http://127.0.0.1:7878';
const MAX = 8;
const $ = s => document.querySelector(s);
const rows = new Map();

const ICON = {
  pause: '<path d="M8 5v14M16 5v14"/>',
  resume: '<path d="M7 4.5v15l12-7.5z"/>',
  openfile: '<path d="M14 4h6v6M20 4l-9 9M18 14v4.5a1.5 1.5 0 0 1-1.5 1.5h-11A1.5 1.5 0 0 1 4 18.5v-11A1.5 1.5 0 0 1 5.5 6H10"/>',
};
const TIP = { pause: 'Pausieren', resume: 'Fortsetzen', openfile: 'Datei öffnen' };
const LABEL = { queue: 'Wartet', wait: 'Verbinde …', pause: 'Pausiert', done: 'Fertig', error: 'Fehler' };
const ORDER = { run: 0, wait: 0, queue: 1, pause: 2, error: 2, done: 3 };

const nf = new Intl.NumberFormat('de-DE', { minimumFractionDigits: 1, maximumFractionDigits: 1 });
function fmtB(b) {
  const u = ['B', 'KB', 'MB', 'GB', 'TB'];
  let i = 0;
  while (b >= 1024 && i < 4) { b /= 1024; i++; }
  return (i ? nf.format(b) : Math.round(b)) + ' ' + u[i];
}
function fmtT(s) {
  s = Math.round(s);
  if (s >= 3600) return Math.floor(s / 3600) + ' h ' + Math.floor(s % 3600 / 60) + ' min';
  if (s >= 60) return Math.floor(s / 60) + ' min';
  return s + ' s';
}

function meta(j) {
  const known = j.size > 0;
  const pct = known ? Math.floor(j.done / j.size * 100) + ' %' : fmtB(j.done);
  if (j.status === 'run') {
    if (j.err) return 'Verbindung unterbrochen, versuche erneut';
    const eta = known && j.speed > 0 ? ' · noch ' + fmtT((j.size - j.done) / j.speed) : '';
    return `${pct} · ${fmtB(j.speed)}/s${eta}`;
  }
  if (j.status === 'done') return 'Fertig · ' + (known ? fmtB(j.size) : fmtB(j.done));
  if (j.status === 'error') return 'Fehler: ' + j.err;
  return LABEL[j.status] + (known && j.done ? ' · ' + pct : '');
}

function row(j) {
  let r = rows.get(j.id);
  if (!r) {
    const li = document.createElement('li');
    const name = document.createElement('div');
    name.className = 'name';
    const btn = document.createElement('button');
    const m = document.createElement('div');
    m.className = 'meta';
    const bar = document.createElement('div');
    bar.className = 'bar';
    bar.append(document.createElement('i'));
    li.append(name, btn, m, bar);
    btn.addEventListener('click', () => act(r.j, btn.dataset.a));
    r = { li, name, btn, m, fill: bar.firstChild, j };
    rows.set(j.id, r);
  }
  r.j = j;
  r.li.className = j.status + (j.status === 'run' && j.err ? ' retry' : '');
  r.name.textContent = r.name.title = j.name;
  r.m.textContent = meta(j);
  r.fill.style.width = (j.size > 0 ? j.done / j.size * 100 : 0) + '%';
  const a = ['run', 'wait', 'queue'].includes(j.status) ? 'pause' : j.status === 'done' ? 'openfile' : 'resume';
  if (r.btn.dataset.a !== a) {
    r.btn.dataset.a = a;
    r.btn.innerHTML = `<svg viewBox="0 0 24 24" aria-hidden="true">${ICON[a]}</svg>`;
    r.btn.title = TIP[a];
    r.btn.setAttribute('aria-label', TIP[a]);
  }
  return r.li;
}

function render(jobs) {
  jobs.sort((a, b) => ORDER[a.status] - ORDER[b.status] || b.id - a.id);
  const shown = jobs.slice(0, MAX);
  const ids = new Set(shown.map(j => j.id));
  for (const id of rows.keys()) if (!ids.has(id)) { rows.get(id).li.remove(); rows.delete(id); }
  const list = $('#list');
  shown.forEach((j, i) => { const li = row(j); if (list.children[i] !== li) list.insertBefore(li, list.children[i] || null); });
  const speed = jobs.reduce((s, j) => s + (j.status === 'run' ? j.speed : 0), 0);
  $('#total').textContent = speed ? fmtB(speed) + '/s' : '';
  $('#empty').hidden = jobs.length > 0;
  $('#more').hidden = jobs.length <= MAX;
  $('#more').textContent = `+ ${jobs.length - MAX} weitere in Speedload`;
}

async function act(j, a) {
  try {
    await fetch(`${SERVER}/api/${a}`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ id: j.id }) });
  } catch {}
  load();
}

async function load() {
  try {
    const jobs = await (await fetch(SERVER + '/api/list', { cache: 'no-store' })).json();
    $('#off').hidden = true;
    render(jobs);
  } catch {
    $('#off').hidden = false;
    $('#empty').hidden = $('#more').hidden = true;
    $('#total').textContent = '';
    rows.forEach(r => r.li.remove());
    rows.clear();
  }
}

$('#open').addEventListener('click', () => { api.tabs.create({ url: SERVER + '/' }); window.close(); });
load();
setInterval(load, 1000);
