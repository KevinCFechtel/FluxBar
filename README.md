# FluxBar

FluxBar zeigt ungelesene Artikel aus Miniflux in der macOS-Menüleiste an. Das Repository enthält zwei getrennte Oberflächen mit einem gemeinsamen Go-Kern:

- `cmd/fluxbar-swiftbar`: Ausgabe im SwiftBar-/xbar-Protokoll
- `cmd/fluxbar-standalone`: eigenständige macOS-Menüleisten-App
- `internal/miniflux`: Miniflux-Abfragen, Markieren als gelesen und Feed-Icon-Cache
- `internal/icons`: Dekodierung und Normalisierung von PNG, JPEG, GIF, WebP, ICO und SVG
- `internal/swiftbar` und `internal/standalone`: die jeweiligen Oberflächen

Feed-Icons werden einmal pro Feed geladen und als 32 × 32 Pixel große PNGs normalisiert. Dadurch verwenden beide Oberflächen exakt dieselben Icon-Daten. Ein Klick auf einen Artikel öffnet ihn im Standardbrowser und markiert ihn in Miniflux als gelesen.

## Konfiguration

```bash
cp miniflux.env.example miniflux.env
```

Anschließend `MINIFLUX_SERVER` und `MINIFLUX_APIKEY` in `miniflux.env` eintragen. Die Datei wird von Git ignoriert. Beide Build-Skripte betten diese Werte in das jeweilige Binary ein. Zur Laufzeit gesetzte Umgebungsvariablen gleichen Namens haben Vorrang.

## Standalone-App

Voraussetzungen sind macOS, Go und die Xcode Command Line Tools. Der native Tray-Code verwendet CGO.

```bash
./standalone/build.sh
open dist/FluxBar.app
```

Die Anwendung erscheint nur in der Menüleiste (`LSUIElement`) und nicht im Dock. Sie aktualisiert Miniflux beim Start, manuell über „Aktualisieren“ und danach alle 15 Minuten. Das Log liegt unter `~/Library/Application Support/FluxBar/fluxbar.log`.

Für einen automatischen Start kann `dist/FluxBar.app` nach `/Applications` kopiert und in macOS unter „Systemeinstellungen → Allgemein → Anmeldeobjekte“ hinzugefügt werden. Für die Verteilung an andere Macs sollte die App mit einer Developer-ID signiert und notarisiert werden; lokal erzeugt das Skript eine Ad-hoc-Signatur.

## SwiftBar

```bash
./swiftbar/build.sh
```

Danach `swiftbar/fluxbar.15m.sh` in den SwiftBar-Plugin-Ordner kopieren oder verlinken. Das Binary muss daneben als `fluxbar.cgo` liegen. Alternativ kann das Skript über `FLUXBAR_BINARY` auf einen anderen Pfad zeigen.

Beispiel mit symbolischen Links:

```bash
ln -s "$PWD/swiftbar/fluxbar.15m.sh" "$HOME/Library/Application Support/SwiftBar/Plugins/fluxbar.15m.sh"
ln -s "$PWD/swiftbar/fluxbar.cgo" "$HOME/Library/Application Support/SwiftBar/Plugins/fluxbar.cgo"
```

SwiftBar erhält für jeden Eintrag ein base64-kodiertes PNG über den Parameter `image=` sowie eine explizite Anzeigegröße von 16 × 16 Punkten.

## Entwicklung

```bash
go test ./...
go vet ./...
```

Die Miniflux-Anbindung und Icon-Verarbeitung sind unabhängig von der Oberfläche testbar. Die Standalone-App basiert bewusst nur auf `fyne.io/systray` und zieht nicht das vollständige Fyne-UI-Toolkit ein.
