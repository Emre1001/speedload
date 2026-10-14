<div align="center">

```
 ___   ___   ___   ___   ___    _       ___      _     ___
/ __| | _ \ | __| | __| |   \  | |     / _ \    /_\   |   \
\__ \ |  _/ | _|  | _|  | |) | | |__  | (_) |  / _ \  | |) |
|___/ |_|   |___| |___| |___/  |____|  \___/  /_/ \_\ |___/
```

**Schneller Download-Manager. Eine Datei, 2 MB, kein Ballast.**

Mehrere Verbindungen pro Download · setzt nach Abbruch fort · drei Designs

![Lizenz](https://img.shields.io/badge/Lizenz-MIT-green)
![Rust](https://img.shields.io/badge/Rust-2021-orange)
![Größe](https://img.shields.io/badge/Binary-~2%20MB-blue)
![Plattformen](https://img.shields.io/badge/Linux%20%C2%B7%20Windows-lightgrey) ![macOS](https://img.shields.io/badge/macOS-bald-555555)

</div>

---

## Warum

Dein Browser lädt eine Datei über **eine** Verbindung. Viele Server drosseln aber jede einzelne. Speedload teilt die Datei in Teile und holt alle gleichzeitig. Das nutzt deine Leitung voll aus.

Fällt das WLAN aus, geht der Strom weg oder stürzt etwas ab, ist nichts verloren. Speedload macht an der Stelle weiter, wo es stand.

## Features

| | |
|---|---|
| **Parallel** | Bis zu 32 Verbindungen pro Datei. Wer früher fertig ist, übernimmt die Hälfte vom langsamsten Teil. |
| **Fortsetzen** | Netz weg: automatischer Neuversuch mit Pausen bis 30 s. Strom weg oder Absturz: Weiter beim nächsten Start. |
| **Sicher gespeichert** | Fortschritt wird jede Sekunde gesichert, erst die Daten, dann der Stand. Nach einem Stromausfall steht nie Fortschritt ohne Daten da. |
| **Leicht** | Eine Datei, ca. 2 MB, keine Laufzeit, kein Electron, kein Webview. |
| **3 Designs** | **Glassmorphism**, **ASCII-Art**, **Split-Flap**. Jedes in Hell und Dunkel, folgt auf Wunsch dem System. Akzentfarbe wählbar. |
| **Flüssig** | Weich mitlaufende Zahlen und Balken, Segment-Ansicht pro Verbindung, Tempo-Graph, animierte Übergänge. Animationen abschaltbar. |
| **Warteschlange** | Bestimmt, wie viele Downloads gleichzeitig laufen. Der Rest wartet und startet automatisch. |
| **Tempolimit** | Gesamtgeschwindigkeit begrenzen, damit für anderes noch Leitung übrig bleibt. |
| **Browser-Übernahme** | Erweiterung für Chrome, Edge, Brave, Firefox und Mullvad Browser übernimmt Downloads automatisch. |
| **Überall** | Linux (Ubuntu, Linux Mint und andere) und Windows. <sub>macOS: <s>folgt später</s></sub> |
| **Privat** | Hört nur auf `127.0.0.1`. Keine Telemetrie, keine Konten, kein Netzwerkzugriff außer deinen Downloads. |

## Designs

Umschalten unter ⚙ Einstellungen:

- **Glassmorphism**: Milchglas-Karten über animiertem Farbverlauf.
- **ASCII-Art**: Terminal-Look in Grün auf Schwarz, Fortschrittsbalken als `[#####==.....]`.
- **Split-Flap**: Klapptafel wie am Bahnhof, jede Ziffer klappt einzeln um.

Farbschema: System, Hell oder Dunkel. Dazu sieben Akzentfarben.

## Einstellungen

| Bereich | Einstellung |
|---|---|
| Darstellung | Design, Farbschema, Akzentfarbe, Animationen (System/Voll/Wenig/Aus), kompakte Liste, Tempo-Graph |
| Downloads | Speicherort, Verbindungen pro Download (1–32), gleichzeitige Downloads (1–10), Tempolimit, beim Start fortsetzen |
| Browser | Übernahme an/aus, Mindestgröße für übernommene Dateien |
| Benachrichtigungen | Desktop-Meldung, Ton bei Fertig |

Bedienhilfen: Link per <kbd>Strg</kbd>+<kbd>V</kbd> irgendwo einfügen oder ins Fenster ziehen, <kbd>/</kbd> springt ins Link-Feld, <kbd>Strg</kbd>+<kbd>,</kbd> öffnet die Einstellungen. Mehrere Links auf einmal gehen auch.

## Installation

### Fertige Programme

Unter [Releases](../../releases) gibt es Programme für Linux und Windows sowie die Browser-Erweiterung. <sub><s>macOS (Intel und Apple Silicon): folgt später.</s></sub>

### Selbst bauen

Du brauchst [Rust](https://rustup.rs). Dann:

```bash
git clone https://github.com/Emre1001/speedload
cd speedload
cargo build --release
```

Ergebnis: `target/release/speedload` (Windows: `speedload.exe`).

### Linux (Ubuntu, Linux Mint, andere)

```bash
./install-linux.sh
```

Installiert nach `~/.local/bin`, legt einen Menüeintrag an und startet Speedload beim Login unsichtbar im Hintergrund (`--hidden`).

### Windows

`speedload.exe` starten. Für Autostart eine Verknüpfung mit `--hidden` in `shell:startup` anlegen.

### macOS

<s>Folgt später.</s> Der Code enthält macOS-Pfade, es gibt aber noch keine Programme und keine Tests. Auf eigene Gefahr selbst bauen geht mit `cargo build --release`.

## Browser-Erweiterung

Speedload funktioniert auch ohne Erweiterung: Link in das Feld einfügen, fertig. Mit Erweiterung übernimmt es jeden Browser-Download selbst. Läuft Speedload nicht, lädt der Browser ganz normal weiter.

**Chrome, Edge, Brave**

`chrome://extensions` → Entwicklermodus → „Entpackte Erweiterung laden“ → Ordner `extension`.

**Firefox**

`about:debugging` → „Dieser Firefox“ → „Temporäres Add-on laden“ → `extension/manifest.json`. Dauerhaft geht das nur mit Firefox Developer Edition oder ESR (siehe Mullvad).

**Mullvad Browser** (dauerhaft)

1. `speedload.xpi` erzeugen: `./install-linux.sh` oder `cd extension && python3 -m zipfile -c ../speedload.xpi manifest.json background.js`. Fertige `.xpi` gibt es auch bei den Releases.
2. `about:config` → `xpinstall.signatures.required` auf `false`.
3. `about:addons` → Zahnrad → „Add-on aus Datei installieren“ → `speedload.xpi`.
4. Bei Speedload „In privaten Fenstern ausführen“ auf **Erlauben** stellen. Pflicht: Mullvad Browser läuft immer im privaten Modus.

> **Datenschutz-Hinweis:** Übernommene Downloads lädt Speedload selbst, nicht der Browser. Sie laufen über die normale Internetverbindung des Systems (mit Mullvad VPN: über das VPN), aber ohne den Fingerprinting-Schutz des Browsers. Speedload sendet die Kennung `Speedload/0.1`.

## Bedienung

Beim Start öffnet sich die Oberfläche als App-Fenster (Chrome, Edge, Brave, Chromium) oder im Standardbrowser unter `http://127.0.0.1:7878`. Ein zweiter Start öffnet nur das Fenster.

⏻ beendet Speedload. Laufende Downloads gehen beim nächsten Start weiter.

Daten und Einstellungen liegen hier:

| System | Pfad |
|---|---|
| Linux | `~/.local/share/speedload` |
| Windows | `%APPDATA%\speedload` |
| macOS (ungetestet) | `~/Library/Application Support/speedload` |

## So funktioniert es

```
 Datei (z. B. 800 MB)
 ├─ Teil 1 ──► Verbindung 1 ─┐
 ├─ Teil 2 ──► Verbindung 2 ─┤
 ├─ ...                      ├─► Datei.part ─► fertig: Datei
 └─ Teil 8 ──► Verbindung 8 ─┘
```

1. Speedload fragt den Server mit `Range: bytes=0-0` nach Größe und Range-Unterstützung.
2. Die Datei wird in bis zu 32 Teile geteilt, jeder Teil läuft über eine eigene Verbindung.
3. Ist ein Teil fertig, teilt dieser Worker das größte verbleibende Stück und hilft dort mit.
4. Fortschritt steht pro Teil in `jobs/<id>.json`. Beim Start liest Speedload alle offenen Jobs ein und macht weiter.
5. Am Ende wird `Datei.part` in den echten Namen umbenannt.

## Grenzen

- **Kein Range-Support am Server:** nur eine Verbindung, Fortsetzen beginnt von vorn.
- **Verbindungslimit am Server:** Manche Server antworten bei vielen Verbindungen mit `429`. Speedload wartet dann und versucht es erneut, schneller wird es nicht. Dann in den Einstellungen weniger Verbindungen wählen.
- **Anmeldung nötig:** Downloads hinter einem Login scheitern, weil keine Cookies weitergegeben werden. `blob:`- und `data:`-Downloads bleiben beim Browser.
- **Nur http(s).** Kein FTP, kein Torrent, keine Streams (HLS/DASH).
- **Windows:** Gebaut und getestet wird in GitHub Actions, Praxistests auf echten Windows-Rechnern fehlen noch. **macOS:** noch nicht unterstützt. Fehler bitte als Issue melden.

## Entwicklung

```bash
cargo test            # Namens-Parser und ein Test: Download abbrechen, neu starten, fortsetzen, Datei byte-genau prüfen
cargo run             # startet Speedload mit Fenster
```

Aufbau:

| Pfad | Inhalt |
|---|---|
| `src/main.rs` | Download-Engine, HTTP-API, Start |
| `ui/index.html` | Oberfläche mit allen drei Designs, ohne Build-Schritt |
| `extension/` | Browser-Erweiterung (Manifest V3) |

Die lokale API (`/api/add`, `/api/list`, `/api/pause`, `/api/resume`, `/api/remove`, `/api/settings`) akzeptiert nur Anfragen von `127.0.0.1` und Browser-Erweiterungen. Webseiten bekommen `403`.

Beiträge sind willkommen: Fork, Branch, `cargo test`, Pull Request.

## Lizenz

[MIT](LICENSE): frei nutzbar, veränderbar und weitergebbar.
