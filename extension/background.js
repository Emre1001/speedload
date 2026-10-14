// Fängt Browser-Downloads ab und gibt sie an Speedload. Läuft Speedload nicht, lädt der Browser normal.
const api = globalThis.browser ?? chrome;
const SERVER = 'http://127.0.0.1:7878';

api.downloads.onCreated.addListener(async (item) => {
  const url = item.finalUrl || item.url;
  // Chrome meldet beim Laden der Erweiterung auch alte Einträge aus dem Download-Verlauf.
  // Nur frisch gestartete, laufende Downloads übernehmen.
  const age = Date.now() - Date.parse(item.startTime);
  if (item.state !== 'in_progress' || !(age < 30000)) return;
  if (!/^https?:/i.test(url) || url.startsWith(SERVER)) return; // blob:/data: kann nur der Browser selbst
  try {
    const r = await fetch(SERVER + '/api/add', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ url, referrer: item.referrer || '', source: 'browser', size: Math.max(item.totalBytes || 0, item.fileSize || 0) }),
    });
    if (!r.ok) return; // abgelehnt (zu klein, Übernahme aus): Browser lädt weiter
    await api.downloads.cancel(item.id);
    await api.downloads.erase({ id: item.id });
  } catch {
    // Speedload nicht erreichbar: Browser-Download weiterlaufen lassen
  }
});
