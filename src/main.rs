#![cfg_attr(windows, windows_subsystem = "windows")]
//! Speedload: lädt Dateien über mehrere parallele Verbindungen (HTTP Range) und setzt
//! nach Netz- oder Stromausfall fort. Lokaler Server auf 127.0.0.1:7878 liefert UI + API.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const ADDR: &str = "127.0.0.1:7878";
const UI: &str = include_str!("../ui/index.html");
const MIN_SPLIT: u64 = 1 << 20; // Reste unter 1 MiB nicht weiter aufteilen
const UNKNOWN: u64 = u64::MAX;

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Seg {
    start: u64,
    pos: u64,
    end: u64,
}

#[derive(Serialize, Deserialize, Clone)]
struct Meta {
    id: u64,
    url: String,
    #[serde(default)]
    referer: String,
    name: String,
    dir: PathBuf,
    size: u64,
    ranges: bool,
    segs: Vec<Seg>, // leer = Server noch nicht geprüft
    status: String, // wait | run | pause | done | error
}

#[derive(Serialize, Deserialize, Clone)]
struct Settings {
    theme: String,
    conns: usize,
    dir: PathBuf,
}

struct Inner {
    meta: Meta,
    file: Option<File>,
}

#[derive(Default)]
struct Flags {
    stop: AtomicBool,
    running: AtomicBool,
    removed: AtomicBool,
    failed: AtomicBool,
}

struct Job {
    inner: Mutex<Inner>,
    f: Flags,
    speed: AtomicU64,
    err: Mutex<String>,
}

struct App {
    jobs: Mutex<Vec<Arc<Job>>>,
    cfg: Mutex<Settings>,
    data: PathBuf,
    agent: ureq::Agent,
    quitting: AtomicBool,
}

fn main() {
    let Ok(server) = tiny_http::Server::http(ADDR) else {
        open_ui(); // läuft schon: nur Fenster öffnen
        return;
    };
    let app = new_app(data_dir());
    if !std::env::args().any(|a| a == "--hidden") {
        open_ui();
    }
    for rq in server.incoming_requests() {
        handle(&app, rq);
    }
}

fn new_app(data: PathBuf) -> Arc<App> {
    let _ = fs::create_dir_all(data.join("jobs"));
    let cfg = fs::read(data.join("settings.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_else(|| Settings { theme: "glass".into(), conns: 8, dir: download_dir() });
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(15))
        .timeout_read(Duration::from_secs(30)) // tote Verbindung (WLAN weg) wird erkannt und neu aufgebaut
        .user_agent("Mozilla/5.0 Speedload/0.1")
        .build();
    let app = Arc::new(App { jobs: Mutex::new(vec![]), cfg: Mutex::new(cfg), data, agent, quitting: AtomicBool::new(false) });
    let mut metas: Vec<Meta> = fs::read_dir(app.data.join("jobs"))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .filter_map(|e| serde_json::from_slice(&fs::read(e.path()).ok()?).ok())
        .collect();
    metas.sort_by_key(|m| m.id);
    for m in metas {
        let resume = m.status == "run" || m.status == "wait"; // war beim Beenden/Absturz aktiv
        let job = new_job(m);
        app.jobs.lock().unwrap().push(job.clone());
        if resume {
            start(&app, job);
        }
    }
    app
}

fn new_job(meta: Meta) -> Arc<Job> {
    Arc::new(Job { inner: Mutex::new(Inner { meta, file: None }), f: Flags::default(), speed: AtomicU64::new(0), err: Mutex::new(String::new()) })
}

// ---------- Download-Engine ----------

fn start(app: &Arc<App>, job: Arc<Job>) {
    if job.f.running.swap(true, Relaxed) {
        return;
    }
    job.f.stop.store(false, Relaxed);
    job.f.failed.store(false, Relaxed);
    let app = app.clone();
    thread::spawn(move || {
        let r = download(&app, &job);
        {
            let mut g = job.inner.lock().unwrap();
            match r {
                Ok(true) => {
                    if let Some(f) = g.file.take() {
                        let _ = f.set_len(g.meta.size);
                        let _ = f.sync_all();
                    }
                    let target = unique(&g.meta.dir, &g.meta.name, false);
                    match fs::rename(part_path(&g.meta), g.meta.dir.join(&target)) {
                        Ok(()) => {
                            g.meta.name = target;
                            g.meta.status = "done".into();
                            job.err.lock().unwrap().clear();
                        }
                        Err(e) => fail(&mut g, &job, format!("Umbenennen fehlgeschlagen: {e}")),
                    }
                }
                Ok(false) if !app.quitting.load(Relaxed) => g.meta.status = "pause".into(),
                Ok(false) => {} // Programmende: Status "run" bleibt, nächster Start macht weiter
                Err(e) => fail(&mut g, &job, e),
            }
            g.file = None;
        }
        save(&app, &job);
        job.speed.store(0, Relaxed);
        job.f.running.store(false, Relaxed);
        if job.f.removed.load(Relaxed) {
            cleanup(&app, &job);
        }
    });
}

fn fail(g: &mut Inner, job: &Job, e: String) {
    g.meta.status = "error".into();
    *job.err.lock().unwrap() = e;
}

/// Ok(true) = fertig, Ok(false) = gestoppt.
fn download(app: &App, job: &Arc<Job>) -> Result<bool, String> {
    let conns = app.cfg.lock().unwrap().conns.clamp(1, 32);
    let (url, referer, probed) = {
        let g = job.inner.lock().unwrap();
        (g.meta.url.clone(), g.meta.referer.clone(), !g.meta.segs.is_empty())
    };
    if !probed {
        let p = probe(&app.agent, &url, &referer)?;
        let mut g = job.inner.lock().unwrap();
        fs::create_dir_all(&g.meta.dir).map_err(|e| e.to_string())?;
        g.meta.name = unique(&g.meta.dir, &p.name, true);
        g.meta.size = p.size;
        g.meta.ranges = p.ranges;
        g.meta.segs = vec![Seg { start: 0, pos: 0, end: p.size }];
    }
    {
        let mut g = job.inner.lock().unwrap();
        fs::create_dir_all(&g.meta.dir).map_err(|e| e.to_string())?;
        let path = part_path(&g.meta);
        let f = OpenOptions::new().create(true).read(true).write(true).truncate(false).open(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let m = &mut g.meta;
        if !m.ranges {
            m.segs[0].pos = 0; // ohne Range-Support nur Neustart möglich
        } else if f.metadata().map(|x| x.len()).unwrap_or(0) != m.size {
            f.set_len(m.size).map_err(|e| format!("Kein Platz? {e}"))?;
        }
        if m.segs.iter().any(|s| s.pos < s.end) {
            m.segs.retain(|s| s.pos < s.end);
        }
        while m.segs.len() < conns && split(m).is_some() {}
        m.status = "run".into();
        g.file = Some(f);
    }
    save(app, job);

    let n = job.inner.lock().unwrap().meta.segs.len();
    let workers: Vec<_> = (0..n)
        .map(|i| {
            let (job, agent) = (job.clone(), app.agent.clone());
            thread::spawn(move || worker(&job, &agent, i))
        })
        .collect();
    let mut last = done_bytes(&job.inner.lock().unwrap().meta);
    while workers.iter().any(|w| !w.is_finished()) {
        thread::sleep(Duration::from_secs(1));
        let now = done_bytes(&job.inner.lock().unwrap().meta);
        job.speed.store(now.saturating_sub(last), Relaxed);
        last = now;
        save(app, job);
    }
    for w in workers {
        let _ = w.join();
    }
    if job.f.failed.load(Relaxed) {
        return Err(job.err.lock().unwrap().clone());
    }
    if job.f.stop.load(Relaxed) {
        return Ok(false);
    }
    let g = job.inner.lock().unwrap();
    if g.meta.segs.iter().all(|s| s.pos >= s.end) {
        Ok(true)
    } else {
        Err("Download unvollständig".into())
    }
}

/// Teilt das Segment mit dem größten Rest. Neuer Index für den freien Worker.
fn split(m: &mut Meta) -> Option<usize> {
    if !m.ranges {
        return None;
    }
    let (j, rest) = m.segs.iter().enumerate().map(|(j, s)| (j, s.end.saturating_sub(s.pos))).max_by_key(|&(_, r)| r)?;
    if rest < MIN_SPLIT {
        return None;
    }
    let mid = m.segs[j].end - rest / 2;
    let end = std::mem::replace(&mut m.segs[j].end, mid);
    m.segs.push(Seg { start: mid, pos: mid, end });
    Some(m.segs.len() - 1)
}

fn worker(job: &Job, agent: &ureq::Agent, mut i: usize) {
    let mut wait = 1;
    let mut buf = vec![0u8; 64 * 1024];
    while !job.f.stop.load(Relaxed) {
        let (url, referer, ranges, pos, end) = {
            let mut g = job.inner.lock().unwrap();
            let m = &mut g.meta;
            if m.segs[i].pos >= m.segs[i].end {
                match split(m) {
                    Some(j) => {
                        i = j; // Arbeit vom langsamsten Segment übernehmen
                        continue;
                    }
                    None => return,
                }
            }
            if !m.ranges {
                m.segs[i].pos = 0;
            }
            (m.url.clone(), m.referer.clone(), m.ranges, m.segs[i].pos, m.segs[i].end)
        };
        let mut req = agent.get(&url);
        if !referer.is_empty() {
            req = req.set("Referer", &referer);
        }
        if ranges {
            req = req.set("Range", &format!("bytes={pos}-{}", end - 1));
        }
        let res = req.call().map_err(|e| e.to_string()).and_then(|r| {
            if ranges && r.status() != 206 {
                Err(format!("Server ignoriert Range (HTTP {})", r.status()))
            } else {
                Ok(r)
            }
        });
        let mut rd = match res {
            Ok(r) => r.into_reader(),
            Err(e) => {
                retry(job, &mut wait, e);
                continue;
            }
        };
        loop {
            if job.f.stop.load(Relaxed) {
                return;
            }
            let n = match rd.read(&mut buf) {
                Ok(0) => {
                    if end == UNKNOWN {
                        let mut g = job.inner.lock().unwrap();
                        let p = g.meta.segs[i].pos;
                        g.meta.segs[i].end = p;
                        g.meta.size = p;
                    }
                    break; // Antwort zu Ende: Rest (falls vorhanden) neu anfragen
                }
                Ok(n) => n,
                Err(e) => {
                    retry(job, &mut wait, e.to_string());
                    break;
                }
            };
            let mut g = job.inner.lock().unwrap();
            let Inner { meta, file } = &mut *g;
            let s = &mut meta.segs[i];
            let take = (s.end - s.pos).min(n as u64) as usize; // Segment kann inzwischen geteilt sein
            let f = file.as_mut().unwrap();
            if let Err(e) = f.seek(SeekFrom::Start(s.pos)).and_then(|_| f.write_all(&buf[..take])) {
                *job.err.lock().unwrap() = format!("Schreibfehler: {e}");
                job.f.failed.store(true, Relaxed);
                job.f.stop.store(true, Relaxed);
                return;
            }
            s.pos += take as u64;
            if wait > 1 {
                wait = 1;
                job.err.lock().unwrap().clear();
            }
            if take < n || s.pos >= s.end {
                break;
            }
        }
    }
}

fn retry(job: &Job, wait: &mut u64, e: String) {
    *job.err.lock().unwrap() = e;
    for _ in 0..*wait * 10 {
        if job.f.stop.load(Relaxed) {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
    *wait = (*wait * 2).min(30);
}

struct Probe {
    size: u64,
    ranges: bool,
    name: String,
}

fn probe(agent: &ureq::Agent, url: &str, referer: &str) -> Result<Probe, String> {
    let mut req = agent.get(url).set("Range", "bytes=0-0");
    if !referer.is_empty() {
        req = req.set("Referer", referer);
    }
    let r = req.call().map_err(|e| e.to_string())?;
    let name = r.header("Content-Disposition").and_then(cd_name).unwrap_or_else(|| url_name(r.get_url()));
    let total = r.header("Content-Range").and_then(|v| v.rsplit('/').next()?.trim().parse().ok());
    let (size, ranges) = match (r.status(), total) {
        (206, Some(n)) => (n, true),
        _ => (r.header("Content-Length").filter(|_| r.status() == 200).and_then(|v| v.parse().ok()).unwrap_or(UNKNOWN), false),
    };
    Ok(Probe { size, ranges, name: sanitize(&name) })
}

fn done_bytes(m: &Meta) -> u64 {
    if m.size == UNKNOWN || m.segs.is_empty() {
        return m.segs.iter().map(|s| s.pos).sum();
    }
    m.size - m.segs.iter().map(|s| s.end.saturating_sub(s.pos)).sum::<u64>().min(m.size)
}

fn save(app: &App, job: &Job) {
    if job.f.removed.load(Relaxed) {
        return;
    }
    let g = job.inner.lock().unwrap();
    if let Some(f) = &g.file {
        let _ = f.sync_data(); // erst Daten auf Platte, dann Fortschritt: nach Stromausfall nie Fortschritt ohne Daten
    }
    let bytes = serde_json::to_vec(&g.meta).unwrap();
    let p = app.data.join("jobs").join(format!("{}.json", g.meta.id));
    drop(g);
    let tmp = p.with_extension("tmp");
    let _ = File::create(&tmp).and_then(|mut f| f.write_all(&bytes).and_then(|_| f.sync_all())).and_then(|_| fs::rename(&tmp, &p));
}

fn cleanup(app: &App, job: &Job) {
    let g = job.inner.lock().unwrap();
    if g.meta.status != "done" {
        let _ = fs::remove_file(part_path(&g.meta));
    }
    let _ = fs::remove_file(app.data.join("jobs").join(format!("{}.json", g.meta.id)));
}

fn part_path(m: &Meta) -> PathBuf {
    m.dir.join(format!("{}.part", m.name))
}

// ---------- Dateinamen ----------

fn cd_name(v: &str) -> Option<String> {
    let mut plain = None;
    for p in v.split(';').map(str::trim) {
        let low = p.to_ascii_lowercase();
        if low.starts_with("filename*=") {
            return Some(pct(p[10..].trim_matches('"').rsplit('\'').next()?));
        }
        if low.starts_with("filename=") {
            plain = Some(p[9..].trim_matches('"').to_string());
        }
    }
    plain
}

fn url_name(u: &str) -> String {
    pct(u.split(['?', '#']).next().unwrap_or("").trim_end_matches('/').rsplit('/').next().unwrap_or(""))
}

fn pct(s: &str) -> String {
    let b = s.as_bytes();
    let (mut out, mut i) = (Vec::new(), 0);
    while i < b.len() {
        if b[i] == b'%' {
            if let Some(h) = s.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(h);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn sanitize(n: &str) -> String {
    let s: String = n.chars().map(|c| if c.is_control() || "/\\:*?\"<>|".contains(c) { '_' } else { c }).collect();
    let s = s.trim().trim_matches('.').to_string();
    if s.is_empty() { "download".into() } else { s }
}

/// Freier Name im Ordner; mit `part` zählt auch eine vorhandene .part-Datei als belegt.
fn unique(dir: &Path, name: &str, part: bool) -> String {
    let taken = |n: &str| dir.join(n).exists() || (part && dir.join(format!("{n}.part")).exists());
    if !taken(name) {
        return name.into();
    }
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s, format!(".{e}")),
        _ => (name, String::new()),
    };
    (1..).map(|i| format!("{stem} ({i}){ext}")).find(|n| !taken(n)).unwrap()
}

// ---------- HTTP-API ----------

fn handle(app: &Arc<App>, mut rq: tiny_http::Request) {
    let hdr = |n: &'static str| rq.headers().iter().find(|h| h.field.equiv(n)).map(|h| h.value.to_string());
    // Schutz gegen fremde Webseiten (CSRF) und DNS-Rebinding
    let host_ok = hdr("Host").is_some_and(|h| h == ADDR || h == "localhost:7878");
    let origin_ok = hdr("Origin").is_none_or(|o| {
        o == format!("http://{ADDR}") || o == "http://localhost:7878" || ["chrome-extension://", "moz-extension://", "extension://"].iter().any(|p| o.starts_with(p))
    });
    let (code, ctype, body) = if !host_ok || !origin_ok {
        (403, "text/plain", "verboten".to_string())
    } else {
        let mut raw = String::new();
        let _ = rq.as_reader().take(1 << 20).read_to_string(&mut raw);
        let v: Value = serde_json::from_str(&raw).unwrap_or_default();
        let url = rq.url().split('?').next().unwrap_or("").to_string();
        match (rq.method().as_str(), url.as_str()) {
            ("GET", "/") => (200, "text/html; charset=utf-8", UI.to_string()),
            (m, p) if p.starts_with("/api/") => match api(app, m, &p[5..], &v) {
                Ok(v) => (200, "application/json", v.to_string()),
                Err(e) => (400, "application/json", json!({ "error": e }).to_string()),
            },
            _ => (404, "text/plain", "nicht gefunden".into()),
        }
    };
    let h = tiny_http::Header::from_bytes("Content-Type", ctype).unwrap();
    let _ = rq.respond(tiny_http::Response::from_string(body).with_status_code(code).with_header(h));
}

fn api(app: &Arc<App>, method: &str, path: &str, v: &Value) -> Result<Value, String> {
    let job = || {
        let id = v["id"].as_u64().ok_or("id fehlt")?;
        app.jobs.lock().unwrap().iter().find(|j| j.inner.lock().unwrap().meta.id == id).cloned().ok_or("unbekannt".to_string())
    };
    match (method, path) {
        ("GET", "list") => Ok(Value::Array(app.jobs.lock().unwrap().iter().map(|j| job_json(j)).collect())),
        ("GET", "settings") => Ok(json!(*app.cfg.lock().unwrap())),
        ("POST", "settings") => {
            let mut s: Settings = serde_json::from_value(v.clone()).map_err(|e| e.to_string())?;
            if !["glass", "ascii", "flap"].contains(&s.theme.as_str()) {
                return Err("unbekanntes Design".into());
            }
            s.conns = s.conns.clamp(1, 32);
            fs::create_dir_all(&s.dir).map_err(|e| format!("Ordner: {e}"))?;
            let _ = fs::write(app.data.join("settings.json"), serde_json::to_vec(&s).unwrap());
            *app.cfg.lock().unwrap() = s;
            Ok(json!({ "ok": true }))
        }
        ("POST", "add") => {
            let url = v["url"].as_str().unwrap_or("").trim();
            if !(url.starts_with("http://") || url.starts_with("https://")) {
                return Err("Nur http(s)-Links".into());
            }
            Ok(json!({ "id": add(app, url, v["referrer"].as_str().unwrap_or("")) }))
        }
        ("POST", "pause") => {
            job()?.f.stop.store(true, Relaxed);
            Ok(json!({ "ok": true }))
        }
        ("POST", "resume") => {
            start(app, job()?);
            Ok(json!({ "ok": true }))
        }
        ("POST", "remove") => {
            let j = job()?;
            j.f.removed.store(true, Relaxed);
            j.f.stop.store(true, Relaxed);
            app.jobs.lock().unwrap().retain(|x| !Arc::ptr_eq(x, &j));
            if !j.f.running.load(Relaxed) {
                cleanup(app, &j);
            }
            Ok(json!({ "ok": true }))
        }
        ("POST", "open") => {
            opener(&job()?.inner.lock().unwrap().meta.dir.to_string_lossy());
            Ok(json!({ "ok": true }))
        }
        ("POST", "quit") => {
            app.quitting.store(true, Relaxed);
            let jobs = app.jobs.lock().unwrap().clone();
            jobs.iter().for_each(|j| j.f.stop.store(true, Relaxed));
            thread::spawn(move || {
                for _ in 0..50 {
                    if jobs.iter().all(|j| !j.f.running.load(Relaxed)) {
                        break;
                    }
                    thread::sleep(Duration::from_millis(100));
                }
                std::process::exit(0);
            });
            Ok(json!({ "ok": true }))
        }
        _ => Err("unbekannter Befehl".into()),
    }
}

fn add(app: &Arc<App>, url: &str, referer: &str) -> u64 {
    let id = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_micros() as u64;
    let dir = app.cfg.lock().unwrap().dir.clone();
    let meta = Meta { id, url: url.into(), referer: referer.into(), name: sanitize(&url_name(url)), dir, size: UNKNOWN, ranges: false, segs: vec![], status: "wait".into() };
    let job = new_job(meta);
    app.jobs.lock().unwrap().push(job.clone());
    save(app, &job);
    start(app, job);
    id
}

fn job_json(j: &Job) -> Value {
    let g = j.inner.lock().unwrap();
    let m = &g.meta;
    json!({
        "id": m.id, "name": m.name, "url": m.url, "status": m.status,
        "size": if m.size == UNKNOWN { Value::Null } else { json!(m.size) },
        "done": done_bytes(m), "speed": j.speed.load(Relaxed),
        "err": *j.err.lock().unwrap(),
        "segs": m.segs.iter().filter(|s| s.pos < s.end).map(|s| [s.pos, s.end]).collect::<Vec<_>>(),
    })
}

// ---------- Plattform ----------

fn home() -> PathBuf {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from).unwrap_or_else(|| ".".into())
}

fn data_dir() -> PathBuf {
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(home)
    } else if cfg!(target_os = "macos") {
        home().join("Library/Application Support")
    } else {
        std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).unwrap_or_else(|| home().join(".local/share"))
    };
    base.join("speedload")
}

fn download_dir() -> PathBuf {
    #[cfg(target_os = "linux")]
    if let Ok(o) = Command::new("xdg-user-dir").arg("DOWNLOAD").output() {
        let p = PathBuf::from(String::from_utf8_lossy(&o.stdout).trim());
        if p.is_absolute() && p != home() {
            return p;
        }
    }
    home().join("Downloads")
}

fn cmd(prog: &str) -> Command {
    #[allow(unused_mut)]
    let mut c = Command::new(prog);
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(&mut c, 0x0800_0000); // kein Konsolenfenster
    c
}

fn opener(target: &str) {
    let prog = if cfg!(windows) { "explorer" } else if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
    let _ = cmd(prog).arg(target).spawn();
}

/// Öffnet die UI als eigenes App-Fenster (Chromium --app), sonst im Standardbrowser.
fn open_ui() {
    let url = format!("http://{ADDR}/");
    let app_arg = format!("--app={url}");
    if cfg!(windows) && cmd("cmd").args(["/C", "start", "", "msedge", &app_arg]).spawn().is_ok() {
        return;
    }
    if cfg!(target_os = "macos") {
        for b in ["Google Chrome", "Microsoft Edge", "Brave Browser"] {
            if cmd("open").args(["-na", b, "--args", &app_arg]).status().is_ok_and(|s| s.success()) {
                return;
            }
        }
    }
    if cfg!(target_os = "linux") {
        for b in ["google-chrome", "chromium", "chromium-browser", "brave-browser", "microsoft-edge"] {
            if cmd(b).arg(&app_arg).spawn().is_ok() {
                return;
            }
        }
    }
    opener(&url);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert_eq!(cd_name("attachment; filename=\"a b.zip\""), Some("a b.zip".into()));
        assert_eq!(cd_name("attachment; filename*=UTF-8''%C3%A4.iso; filename=x"), Some("ä.iso".into()));
        assert_eq!(url_name("https://h/x/Datei%20Eins.tar.gz?t=1"), "Datei Eins.tar.gz");
        assert_eq!(sanitize("a/b:c"), "a_b_c");
    }

    /// Lokaler Server mit Range-Support, der absichtlich nur 300 KB pro Antwort liefert.
    fn serve(data: Arc<Vec<u8>>) -> String {
        let s = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let port = s.server_addr().to_ip().unwrap().port();
        thread::spawn(move || {
            for rq in s.incoming_requests() {
                let data = data.clone();
                thread::spawn(move || {
                    let r = rq.headers().iter().find(|h| h.field.equiv("Range")).map(|h| h.value.to_string());
                    let (a, b) = r
                        .and_then(|r| {
                            let (a, b) = r.strip_prefix("bytes=")?.split_once('-')?;
                            Some((a.parse::<usize>().ok()?, b.parse::<usize>().ok()?))
                        })
                        .unwrap();
                    let b = b.min(a + 300_000);
                    thread::sleep(Duration::from_millis(150));
                    let cr = tiny_http::Header::from_bytes("Content-Range", format!("bytes {a}-{b}/{}", data.len())).unwrap();
                    let _ = rq.respond(tiny_http::Response::from_data(data[a..=b].to_vec()).with_status_code(206).with_header(cr));
                });
            }
        });
        format!("http://127.0.0.1:{port}/test%20file.bin")
    }

    fn wait_until(app: &App, ok: impl Fn(&Job) -> bool) {
        for _ in 0..600 {
            if app.jobs.lock().unwrap().iter().all(|j| ok(j)) {
                return;
            }
            thread::sleep(Duration::from_millis(100));
        }
        panic!("Timeout");
    }

    #[test]
    fn resumes_after_restart() {
        let data: Arc<Vec<u8>> = Arc::new((0..20_000_000u32).map(|i| (i * 7 % 251) as u8).collect());
        let url = serve(data.clone());
        let dir = std::env::temp_dir().join(format!("speedload-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let s = Settings { theme: "glass".into(), conns: 8, dir: dir.join("out") };
        fs::write(dir.join("settings.json"), serde_json::to_vec(&s).unwrap()).unwrap();

        let app = new_app(dir.clone());
        add(&app, &url, "");
        thread::sleep(Duration::from_millis(900));
        app.quitting.store(true, Relaxed); // wie Programmende mitten im Download
        app.jobs.lock().unwrap().iter().for_each(|j| j.f.stop.store(true, Relaxed));
        wait_until(&app, |j| !j.f.running.load(Relaxed));
        let part = done_bytes(&app.jobs.lock().unwrap()[0].inner.lock().unwrap().meta);
        assert!(part > 0 && part < data.len() as u64, "teilweise geladen: {part}");

        let app2 = new_app(dir.clone()); // lädt Status von Platte und macht weiter
        wait_until(&app2, |j| j.inner.lock().unwrap().meta.status == "done");
        assert_eq!(fs::read(dir.join("out/test file.bin")).unwrap(), *data);
        let _ = fs::remove_dir_all(&dir);
    }
}
