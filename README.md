# FluxBar

FluxBar zeigt ungelesene Artikel aus Miniflux in der macOS-Menüleiste an. Das Repository enthält zwei getrennte Oberflächen mit einem gemeinsamen Go-Kern:

- `cmd/fluxbar-swiftbar`: Ausgabe im SwiftBar-/xbar-Protokoll
- `cmd/fluxbar-standalone`: eigenständige macOS-Menüleisten-App
- `internal/miniflux`: Miniflux-Abfragen, Markieren als gelesen und Feed-Icon-Cache
- `internal/icons`: Dekodierung und Normalisierung von PNG, JPEG, GIF, WebP, ICO und SVG
- `internal/swiftbar` und `internal/standalone`: die jeweiligen Oberflächen

Feed-Icons werden einmal pro Feed geladen und als 32 × 32 Pixel große PNGs normalisiert. Dadurch verwenden beide Oberflächen exakt dieselben Icon-Daten. Ein Klick auf einen Artikel öffnet ihn im Standardbrowser und markiert ihn in Miniflux als gelesen.

Transparente Icons werden anhand ihrer alpha-gewichteten Helligkeit und ihres Kontrasts auf einem dunklen Menühintergrund analysiert. Erkennt FluxBar ein schlecht lesbares dunkles Icon, erzeugt es zusätzlich eine Dark-Mode-Variante mit einer hellgrauen, abgerundeten Fläche. Im Light Mode bleibt das Original unverändert.

## Konfiguration

Die Standalone-App benötigt keine Konfigurationsdatei. Beim ersten Start zeigt sie
„Konfiguration erforderlich“ an. Über „Einstellungen…“ im Menü können Miniflux-URL
und API-Key eingegeben werden. Beide Werte werden gemeinsam als geschützter Eintrag
im macOS-Schlüsselbund gespeichert und überleben App- und Systemneustarts.

Nur die SwiftBar-Variante verwendet weiterhin `miniflux.env`:

```bash
cp miniflux.env.example miniflux.env
```

Anschließend `MINIFLUX_SERVER` und `MINIFLUX_APIKEY` in `miniflux.env` eintragen. Die Datei wird von Git ignoriert. Das SwiftBar-Build-Skript bettet diese Werte in sein Binary ein. Zur Laufzeit gesetzte Umgebungsvariablen gleichen Namens haben Vorrang.

## Standalone-App

Voraussetzungen sind macOS, Go und die Xcode Command Line Tools. Der native Tray-Code verwendet CGO.

```bash
./standalone/build.sh
open dist/FluxBar.app
```

Die Anwendung erscheint nur in der Menüleiste (`LSUIElement`) und nicht im Dock. Sie aktualisiert Miniflux beim Start, manuell über „Aktualisieren“ und danach alle 15 Minuten. Das aktuelle macOS-Erscheinungsbild wird nativ über AppKit erkannt; bei einem Wechsel zwischen Light und Dark Mode wird das Menü automatisch neu aufgebaut. Das Log liegt unter `~/Library/Application Support/FluxBar/fluxbar.log`.

Das App-Bundle verwendet `assets/FluxBarIcon.png` als Ausgangsgrafik für das
vollständige macOS-Icon `standalone/AppIcon.icns`. Das Menüleisten-Symbol bleibt
davon unabhängig ein monochromes Template-Icon.

Die Zugangsdaten lassen sich jederzeit über „Einstellungen…“ ändern. Das native
Einstellungsfenster verwendet für den API-Key ein Passwortfeld. Nach dem Speichern
wird der Go-basierte Miniflux-Client mit den neuen Werten neu erzeugt; laufende
Abfragen mit der alten Konfiguration werden verworfen. Die üblichen macOS-
Tastenkürzel zum Ausschneiden, Kopieren, Einfügen und Auswählen funktionieren in
den Eingabefeldern. Beim Start zeigt FluxBar kurz an, dass die App nun in der
Menüleiste läuft. Diese Startanzeige kann im Einstellungsdialog deaktiviert werden;
die Auswahl wird zusammen mit der übrigen Konfiguration im Schlüsselbund gespeichert.

Für einen automatischen Start kann `dist/FluxBar.app` nach `/Applications` kopiert und in macOS unter „Systemeinstellungen → Allgemein → Anmeldeobjekte“ hinzugefügt werden. Für die Verteilung an andere Macs sollte die App mit einer Developer-ID signiert und notarisiert werden; lokal erzeugt das Skript eine Ad-hoc-Signatur.

### Signiertes und notarisiertes Release

Apple-Zugangsdaten werden nicht im Repository gespeichert. Einmalig wird dafür ein
geschütztes Profil im macOS-Schlüsselbund angelegt. `notarytool` fragt die benötigten
Angaben interaktiv ab; das App-spezifische Passwort erscheint dadurch nicht in der
Shell-History:

```bash
xcrun notarytool store-credentials FluxBar-notary
```

Danach werden Signaturidentität und Name des Schlüsselbundprofils in der lokalen,
durch Git ignorierten Datei `standalone/.env` hinterlegt. Als Vorlage dient
`standalone/.env.example`. Das Release-Skript lädt diese Datei automatisch und führt
Build, Developer-ID-Signatur mit Hardened Runtime, Notarisierung, Stapling und
sämtliche Prüfungen aus:

```bash
./standalone/release.sh
```

Alternativ kann für `SIGNING_IDENTITY` der SHA-1-Fingerabdruck aus
`security find-identity -v -p codesigning` verwendet werden. Die Identität und der
Profilname sind keine Passwörter; Apple-ID, App-spezifisches Passwort beziehungsweise
ein App-Store-Connect-Schlüssel verbleiben vollständig im Schlüsselbund. Bereits in
der Shell gesetzte Werte haben durch die Vorgabewerte in `.env` weiterhin Vorrang.

Standardmäßig verwendet das Skript wegen möglicher IPv6-Probleme eine zur Laufzeit
ermittelte IPv4-Adresse von Apples Zeitstempelserver. Falls nötig, kann der Server
ohne Änderung am Repository überschrieben werden:

```bash
SIGNING_TIMESTAMP_URL='http://timestamp.apple.com/ts01' \
./standalone/release.sh
```

Die Version stammt aus `CFBundleShortVersionString` in `standalone/Info.plist`. Das
fertige Archiv liegt anschließend unter
`dist/release/FluxBar-<Version>-macos-<Architektur>.zip`. Es wird ohne AppleDouble-
Dateien erzeugt und nach dem Verpacken nochmals extrahiert, signaturgeprüft und von
Gatekeeper bewertet.

## SwiftBar

```bash
./swiftbar/build.sh
```

Danach `swiftbar/fluxbar.15m.sh` in den SwiftBar-Plugin-Ordner kopieren oder verlinken. Das Skript sucht das Binary im Ordner `FluxBar` neben dem Plugin-Ordner. Unterstützt werden sowohl `FluxBar/fluxbar.cgo` als auch der Build-Pfad `FluxBar/swiftbar/fluxbar.cgo`. Alternativ kann `FLUXBAR_BINARY` einen anderen Pfad vorgeben.

Beispiel mit symbolischen Links:

```bash
ln -s "$PWD/swiftbar/fluxbar.15m.sh" "$HOME/Library/Application Support/SwiftBar/Plugins/fluxbar.15m.sh"
```

SwiftBar erhält für jeden Eintrag ein base64-kodiertes PNG über den Parameter `image=` sowie eine explizite Anzeigegröße von 16 × 16 Punkten. FluxBar liest die von SwiftBar bereitgestellte Variable `OS_APPEARANCE` und gibt selbst entweder das Original oder die vorbereitete Dark-Mode-Variante aus. Damit ist FluxBar nicht von SwiftBars fehleranfälliger Auswahl zweier kommaseparierter Bilder abhängig. Das monochrome Menüleisten-Logo wird als `templateImage=` ausgegeben, damit macOS es passend zu einer hellen oder dunklen Menüleiste einfärbt.

Der von Miniflux gelieferte HTML-Inhalt wird ohne Skripte, Styles oder Markup in eine Klartextvorschau von höchstens 600 Zeichen umgewandelt. Absätze und Zeilenumbrüche bleiben erhalten; vorhandene Alternativtexte von Bildern werden übernommen. SwiftBar zeigt diese Vorschau als Tooltip des jeweiligen Artikels an. Im xbar-Kompatibilitätsmodus wird kein Tooltip-Parameter ausgegeben, da xbar diesen Parameter nicht unterstützt.

In der Standalone-App erscheint nach 500 ms über einem hervorgehobenen Artikel eine nicht aktivierbare native macOS-Vorschau mit Titel, Feed und Klartext. Sie schließt sich beim Wechsel des Eintrags oder Verlassen des Menüs und stiehlt dem Menü weder Fokus noch Mausereignisse. Enthält der Miniflux-Inhalt ein geeignetes Artikelbild, wird es erst nach Ablauf der Hover-Verzögerung geladen und oberhalb des Texts angezeigt; deklarierte Tracking-Pixel bis 2 × 2 Pixel werden übersprungen. Ein Klick auf den Menüeintrag öffnet immer direkt den Originalartikel im Browser und markiert ihn anschließend als gelesen.

## Entwicklung

```bash
go test ./...
go vet ./...
```

Die Miniflux-Anbindung und Icon-Verarbeitung sind unabhängig von der Oberfläche testbar. Die Standalone-App basiert bewusst nur auf `fyne.io/systray` und zieht nicht das vollständige Fyne-UI-Toolkit ein.

## Icon-Diagnose

Fehler bei einzelnen Feed-Icons werden immer strukturiert protokolliert. Für erfolgreiche Icons, Cache-Treffer und eine Zusammenfassung pro Aktualisierung kann ausführliches Logging aktiviert werden:

```bash
FLUXBAR_DEBUG_ICONS=1 ./dist/FluxBar.app/Contents/MacOS/FluxBar
```

Für SwiftBar müssen diese Variablen in der Umgebung von SwiftBar verfügbar sein. Alternativ können sie am Anfang von `fluxbar.15m.sh` exportiert werden:

```bash
export FLUXBAR_DEBUG_ICONS=1
```

Die Diagnose enthält Feed-ID und -Name, API- und Data-URL-MIME-Typ, den anhand der Dateisignatur erkannten Typ, Datenmenge, einen kurzen SHA-256-Fingerabdruck, Decoderstufe, Dimensionen, SVG-ViewBox und die vollständige Fehlerkette. API-Key und base64-Nutzdaten werden nicht protokolliert.

Die Logs können während einer Aktualisierung direkt beobachtet werden:

```bash
# Standalone
tail -f "$HOME/Library/Application Support/FluxBar/fluxbar.log"

# SwiftBar – bei der Standardstruktur
tail -f "$HOME/Library/Application Support/SwiftBar/FluxBar/swiftbar/fluxbar.log"
```

Der tatsächliche SwiftBar-Logpfad liegt immer neben dem verwendeten `fluxbar.cgo`-Binary. Eine typische Fehlerzeile nennt mit `stage=data_url`, `base64`, `decode`, `svg` oder `resize` die genaue Verarbeitungsstufe. Am Ende folgt eine Zusammenfassung mit der Anzahl erfolgreicher, fehlgeschlagener und aus dem Cache geladener Feed-Icons.

Im Modus `AUTO` wird die helle Fläche nur ergänzt, wenn ein Icon sowohl dunkel ist als auch mindestens 10 % transparente Pixel enthält. Kleinere Alpha-Ränder, wie sie durch geglättete Kanten oder abgerundete Favicon-Ecken entstehen, gelten nicht als inhaltliche Transparenz. Die automatische Hintergrundentscheidung kann für einzelne Feed-IDs überschrieben werden. `ALWAYS` erzwingt die Fläche auch bei einem deckenden Icon; bei Überschneidungen hat `NEVER` Vorrang:

```bash
export FLUXBAR_ICON_BACKGROUND_ALWAYS="114,113"
export FLUXBAR_ICON_BACKGROUND_NEVER="11"
```

Mit aktiviertem Icon-Debug-Logging erscheinen dazu `mean_luminance`, `dark_contrast`, `low_contrast_ratio`, `has_transparency`, `transparent_ratio`, `classified_dark`, `background_mode` und `background_added`.

Optional können ausschließlich fehlgeschlagene, bereits base64-dekodierte Bilder lokal abgelegt werden:

```bash
export FLUXBAR_DUMP_FAILED_ICONS=1
export FLUXBAR_ICON_DUMP_DIR="$HOME/Library/Application Support/FluxBar/icon-debug"
```

Ohne `FLUXBAR_ICON_DUMP_DIR` wird dasselbe Verzeichnis automatisch verwendet. Der Ordner erhält die Rechte `0700`, die Bilddateien `0600`. Da die Dateien Inhalte fremder Feeds enthalten können, sollte diese Option nur vorübergehend zur Diagnose aktiviert werden.
