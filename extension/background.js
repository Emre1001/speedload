// Fängt Browser-Downloads ab und gibt sie an Speedload. Läuft Speedload nicht, lädt der Browser normal.
const api = globalThis.browser ?? chrome;

api.downloads.onCreated.addListener(async (item) => {
  const url = item.finalUrl || item.url;
  if (!/^https?:/i.test(url)) return; // blob:/data: kann nur der Browser selbst
  try {
    const r = await fetch('http://127.0.0.1:7878/api/add', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ url, referrer: item.referrer || '' }),
    });
    if (!r.ok) return;
    await api.downloads.cancel(item.id);
    await api.downloads.erase({ id: item.id });
  } catch {
    // Speedload nicht erreichbar: Browser-Download weiterlaufen lassen
  }
});
