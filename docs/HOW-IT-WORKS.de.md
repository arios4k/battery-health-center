# So funktioniert Battery Health Center

Die Details hinter der App, für Neugierige und Mitwirkende. Zurück zur [README](../README.de.md). Auch auf [English](HOW-IT-WORKS.md), [Español](HOW-IT-WORKS.es.md), [Français](HOW-IT-WORKS.fr.md) und [简体中文](HOW-IT-WORKS.zh-CN.md).

- **Akkudaten** werden direkt aus `/sys/class/power_supply/BAT*` gelesen. Sowohl energiebasierte (µWh) als auch
  ladungsbasierte (µAh) Treiber werden unterstützt.
- **Ladelimits** werden von `bhc-helper` in `charge_control_start_threshold` und `charge_control_end_threshold`
  geschrieben, dem einzigen Teil, der als root läuft. Er verwendet nur die Rust-Standardbibliothek (plus libc für
  die Ortszeit), akzeptiert nur geprüfte Argumente und schreibt ausschließlich diese sysfs-Dateien, seine eigenen
  Konfigurationsdateien in `/etc` (das gespeicherte Limit und den Ladeplan) und, für optimiertes Laden, eine Liste
  von Abziehzeiten in `/var/lib/battery-health-center`. Wenn Sie **Nach Neustart beibehalten** einschalten, wendet
  der systemd-Dienst das Limit beim Start und nach dem Aufwachen erneut an. Unterstützt von ThinkPads, vielen
  Geräten von ASUS, Dell, Huawei, LG, Samsung und System76 sowie anderen mit einem Kerneltreiber für die
  Schwellenwerte.
- **Der Ladeplan** benötigt ein gespeichertes Ladelimit, da er dieses Limit ab der Vorlaufzeit (standardmäßig
  2 Stunden) vor jeder eingestellten Zeit bis 30 Minuten danach auf 100 % aufhebt und es dann wieder setzt. Ein
  systemd-Dienst mit Root-Rechten (`battery-health-center-scheduler`, nur dann von einer `.path`-Unit gestartet,
  wenn ein Plan existiert) erledigt das, ohne jedes Mal nach einem Passwort zu fragen. **Optimiertes Laden**
  zeichnet außerdem auf, wann Sie den Stecker ziehen, nachdem der Laptop mindestens 3 Stunden angesteckt war. Hat
  es in den letzten 28 Tagen mindestens 3 solcher Vorgänge an Werktagen (oder am Wochenende) gesehen, die
  höchstens anderthalb Stunden auseinanderliegen, nutzt es das frühere Ende dieses Bereichs als Bereitzeit.
  Nichts verlässt Ihren Computer.
- **Der Energieverbrauch der Apps** ist eine Schätzung, weil Linux die Leistung nicht pro App misst. Die CPU-Zeit
  wird aus `/proc` abgetastet. Prozesse werden anhand des systemd-Scopes, in dem GNOME jede App startet, zu Apps
  gruppiert, ersatzweise über den übergeordneten Prozess bei Apps, die ihre Prozesse anderswohin verlagern (etwa
  Chromium-basierte Browser). Im Akkubetrieb wird die gemessene Leistung über dem Leerlauf nach CPU-Nutzung auf
  die Apps verteilt.
- **Der Energiemodus** nutzt die D-Bus-API von power-profiles-daemon, dieselbe wie die Schnelleinstellungen von
  GNOME.
- **Der Verlauf** wird, solange die App läuft, einmal pro Minute in
  `~/.local/share/battery-health-center/history.csv` gespeichert. Schalten Sie **Verlauf im Hintergrund
  aufzeichnen** ein, damit sie beim Anmelden unsichtbar startet.
- **Updates und die unsichtbare Kopie.** Die beim Anmelden gestartete Hintergrundkopie würde nach einem Update, das
  die Programmdatei ersetzt, weiter die alte Version ausführen. Sie bemerkt das (Linux markiert die ersetzte Datei
  als gelöscht) und startet, sobald kein Fenster geöffnet ist, die neue Version und beendet sich. Sie prüft einmal
  pro Minute, sodass die neue Version etwa eine Minute nach dem Update läuft.
- **Adaptive Arbeitsbereiche**: Übersicht, Zustand und Nutzung verwenden in größeren Fenstern zwei Spalten. Laden
  und Energie verwenden eine zentrierte einzelne Spalte, damit die Einstellungen in Leserichtung bleiben. Unterhalb
  von 820 logischen Pixeln Arbeitsbereichsbreite werden die Dashboard-Spalten gestapelt; unterhalb von 600
  logischen Pixeln Fensterbreite wird die Seitenleiste zur Zurück-Navigation.
- **Speicherfehler** erscheinen in einem dauerhaften Banner mit dem betroffenen Pfad und der Fehlermeldung. Die
  Einstellungen werden atomar ersetzt und im Speicher wiederhergestellt, wenn das Speichern fehlschlägt.
  Zustandseinträge und abgeschlossene App-Nutzungsstunden werden erneut versucht, sobald der Speicher wieder
  verfügbar ist. Akkumessungen bleiben im Speicher erhalten, wenn ihr Schreiben fehlschlägt.
- **Das App-Symbol** wird aus `data/icons/hicolor/scalable/apps/io.github.BatteryHealthCenter.svg` ausgeliefert;
  `docs/icon.svg` ist die passende Kopie für die Dokumentation. Desktop-Eintrag, PolicyKit-Abfragen,
  Benachrichtigungen und der Info-Dialog verwenden dieselbe Anwendungs-ID. Aus dem Quellcode gebaute Versionen
  laden das Quellsymbol direkt im Info-Dialog, in Benachrichtigungen und in der App-Nutzungsliste, damit ein
  älteres installiertes Symbol es nicht überschreibt.
