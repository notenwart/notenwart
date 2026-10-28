---
title: Verwaltung
parent: Deutsch
nav_order: 5
permalink: /de/verwaltung/
lang: de
---

# Verwaltung

Admins richten die Einrichtung ein, legen Personen an und behalten den Server im Blick. Wie man einen Server
installiert, steht unter [Server betreiben]({{ '/de/betrieb/' | relative_url }}).

## Der erste Start

Wer einen neuen Server als Erstes öffnet, sieht *Willkommen bei notenwart*: Organisation benennen und das erste
Admin-Konto anlegen. Tun Sie das gleich nach der Installation – bis dahin kann es jeder, der die Adresse öffnet.

## Einrichtung

*System* (nur Admins) beginnt mit einer Checkliste; *Meine Noten* erinnert Admins an offene Schritte (*Einrichtung
fortsetzen*).

1. **Name und Logo der Einrichtung** (optional) – auf der Seite *Organisation*; das Logo erscheint in der Kopfzeile
   und auf der Anmeldeseite. Dort legen Sie auch fest, ob *Personen ein Profilbild zeigen dürfen*. Ist es aus, wird
   von niemandem ein Bild gezeigt und keines hochgeladen – bedenkenswert, wenn viele Mitglieder minderjährig sind.
2. **Instrumente prüfen** – siehe [Instrumente]({{ '/de/notenverwaltung/#instrumente' | relative_url }}); von Hand
   abhaken.
3. **Ensembles anlegen** – Chöre, Orchester, Klassen, Kurse (*Ensembles*).
4. **Personen hinzufügen** – siehe unten.
5. **Noten übernehmen** – siehe
   [Einen Ordner importieren]({{ '/de/notenverwaltung/#einen-ordner-importieren' | relative_url }}).
6. **E-Mail** – wird auf dem Server eingestellt; *Test-E-Mail senden* schickt eine an Sie selbst und hakt den Schritt
   ab.
7. **Anmeldung mit Schulkonten** (optional) – wird auf dem Server eingestellt.
8. **Regelmäßig sichern** – auf dem Server; von Hand abhaken.

Unter der Checkliste zeigt *Dieser Server* Version, Adresse, E-Mail-Einstellungen, belegten Speicher, die
Upload-Grenze, wie Noten gelesen werden und ob die Anmeldung mit Schulkonten an ist.

## Personen

*Personen* (nur Admins) listet alle mit Rolle, Ensembles und Status (eingeladen, hat ein Passwort, zuletzt da).

**Eine Person anlegen:** Name, E-Mail, Rolle → *Hinzufügen und einladen*. Verschickt der Server E-Mails, geht die
Einladung per E-Mail hinaus; den Link können Sie immer auch kopieren und selbst weitergeben. Er funktioniert einmal,
sieben Tage lang; *Neuer Anmeldelink* erzeugt einen neuen.

**Viele Personen auf einmal:** *Aus einer Tabelle importieren* nimmt eine CSV-Datei – gespeichert aus Excel,
LibreOffice oder Numbers; *Vorlage herunterladen* zeigt die Spalten: Name und E-Mail, wahlweise Rolle, Ensemble,
Instrument und Stimmnummer (deutsche oder englische Spaltennamen; Vor- und Nachname werden zusammengefügt). Jede Zeile
wird zuerst geprüft; geändert wird nur, wenn alle Zeilen in Ordnung sind. Fehlende Ensembles werden angelegt,
vorhandene Personen nur Ensembles zugeordnet. Neue Personen bekommen Einladungen; *Einladungslinks herunterladen
(CSV)* liefert die Links zum Verteilen. Bis zu 2000 Zeilen auf einmal.

**Mehrere Personen markieren** in der Tabelle: *Zu Ensemble hinzufügen …* (mit Instrument und Stimmnummer), *Aus
Ensemble entfernen …*, *Rolle ändern …*, *Neue Einladungslinks* (als CSV) oder löschen.

**Rollen:** Mitglied, Leitung, Notenverwaltung, Admin – siehe die [Übersicht]({{ '/de/' | relative_url }}). Admins
können sich die eigene Admin-Rolle nicht nehmen und sich nicht selbst löschen.

## Datenschutz

- **Export:** *Daten exportieren* auf der Personen-Seite liefert alles, was über eine Person gespeichert ist, als
  Datei (und *Meine Daten herunterladen* auf der eigenen Kontoseite) – für Anfragen nach Art. 15 und 20 DSGVO.
- **Löschen** einer Person entfernt ihr Konto mit Mitgliedschaften, Anmeldungen, Eintragungen und einzelnen Stimmen.
  Personen mit noch ausgeliehenen Noten lassen sich erst nach der Rückgabe löschen. Jede Löschung steht im
  *Löschprotokoll* – wann, durch wen, wie viele Datensätze – ohne Name oder E-Mail der Person.
- **Nach einem Schuljahr:** *Ehemalige auswählen* markiert Personen, die in keinem Ensemble sind und sich seit einem
  Jahr nicht angemeldet haben (nie Admins), um sie zu prüfen und auf einmal zu löschen.

notenwart speichert Namen, E-Mail-Adressen, Ensembles, Instrumente und den Zeitpunkt der letzten Anmeldung – kein
Tracking, keine Telemetrie. Die Einrichtung, die einen Server betreibt, ist für die Daten darauf verantwortlich und
für die Rechte an den Noten, die dort geteilt werden.
