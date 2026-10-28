---
title: Notenverwaltung
parent: Deutsch
nav_order: 4
permalink: /de/notenverwaltung/
lang: de
---

# Notenverwaltung

Die Notenverwaltung (und Admins) pflegt die Bibliothek: Werke, ihre PDFs und Stimmen, die Instrumentenliste und das
Papierarchiv. Außerdem legt sie Ensembles an, ändert und löscht sie und bestimmt, wer sie leitet.

## Werke

*Bibliothek* zeigt alle Werke als Tabelle: nach einer Spalte sortieren, Spalten wählen, nach Titel, Komponist,
Schlagwort und mehr filtern, Seiten mit 25 bis 100 Einträgen – je Gerät gemerkt. Mehrere Werke markieren, um sie
*Einem Ensemble geben …* oder zu löschen.

*Titel eines neuen Werks* und *Werk anlegen* legen eines an. Auf seiner Seite *Details bearbeiten*: Titel, Komponist,
Arrangeur, Texter, Verlag, Besetzung, Schwierigkeit, Dauer (Min:Sek), Schlagwörter, Standort im Archiv, Werknummer
(GEMA), Quelle / Lizenz, Notizen. Beim Tippen werden Namen und Schlagwörter vorgeschlagen, die es in der Bibliothek
schon gibt.

## PDFs und Stimmen

Ein Werk hat **Dateien** – die PDFs, wie sie hochgeladen wurden – und **Stimmen**, jede aus Seiten einer Datei.

1. **Hochladen:** PDFs auf die Seite des Werks ziehen oder auswählen; mehrere auf einmal gehen. Dieselbe Datei
   zweimal wird abgelehnt. Der Server begrenzt die Größe je Datei (200 MB, wenn nicht anders eingestellt).
2. **Angaben aus der PDF:** Die erste Datei eines Werks ohne Komponist, Arrangeur und Verlag wird gleich gelesen;
   *Angaben aus der PDF lesen* tut es auf Wunsch. notenwart bietet das Gefundene Feld für Feld an – *Übernehmen* oder
   *Verwerfen*; Felder mit Inhalt werden nur ersetzt, wenn Sie sie ankreuzen.
3. **In Stimmen aufteilen:** für eine Datei mit mehreren Stimmen:
   - *Stimmen erkennen* liest den oberen Rand jeder Seite und schlägt die Stimmen vor („2. Trompete in B“,
     „Direktion“): Eine neue Überschrift beginnt eine Stimme, Seiten ohne setzen sie fort. Prüfen, umbenennen,
     Instrument oder Nummer ändern, einen Vorschlag Seite für Seite *Anpassen* und die angekreuzten auf einmal
     anlegen.
   - Oder von Hand: Seiten anklicken (mit Umschalt einen Bereich, in beliebiger Reihenfolge), Instrument,
     Stimmnummer und Bezeichnung angeben, *Stimme anlegen*. Seiten, die schon in anderen Stimmen stecken, sind
     markiert.

   Eine Datei, die nur eine Stimme ist, wird eine Stimme mit allen Seiten. Eine Stimme ohne Instrument ist eine
   Partitur – für die Leitung.

Die Erkennung funktioniert bei digitalen PDFs und bei Scans (Texterkennung). Sie schlägt nur vor – Sie sehen und
korrigieren alles, bevor es gespeichert wird. Ihr Server kann für schlechte Scans zusätzlich eine externe KI nutzen;
siehe [Server betreiben]({{ '/de/betrieb/#noten-erkennen' | relative_url }}).

## Einen Ordner importieren

Um eine bestehende Sammlung zu übernehmen – etwa aus Google Drive (Rechtsklick auf den Ordner → *Herunterladen*
ergibt eine ZIP-Datei) – in der Bibliothek *Ordner importieren …* öffnen und eine ZIP-Datei, PDFs oder einen Ordner
hineinziehen.

- Jeder Ordner mit PDFs wird ein **Werk** mit dem Namen des Ordners; die Ordner darüber werden **Schlagwörter**. PDFs
  direkt im gewählten Ordner werden eigene Werke.
- Eine Datei mit einem Instrument im Namen („Trompete 2.pdf“) wird diese Stimme, eine, die nach Partitur aussieht
  („Partitur“, „Score“), eine Stimme ohne Instrument; alles andere wird nur hochgeladen und später auf der Seite des
  Werks aufgeteilt.
- *Aus dem Inhalt erkennen* liest zusätzlich die erste Seite jeder PDF – Titel, Komponist, Arrangeur, Verlag, die
  Stimme. Nennt der Dateiname die Stimme, bleibt er maßgeblich; für den Titel eines Ordners macht die Seite nur einen
  Vorschlag (*Diesen Titel übernehmen* oder *Ordnernamen behalten*).
- **Werke, die es schon gibt** (gleicher Titel), werden weggelassen, bis Sie entscheiden: *Dateien zum vorhandenen
  Werk hinzufügen* oder *Eigenes Werk anlegen*.
- In der Vorschau ändern Sie Titel, Schlagwörter, was aus jeder Datei wird, Instrument, Nummer und Bezeichnung, oder
  lassen etwas weg. Zu große Dateien und Nicht-PDFs werden als übersprungen aufgeführt.

Der Import läuft Datei für Datei; lassen Sie die Seite geöffnet, bis er abgeschlossen ist. Dateien, die schon in der
Bibliothek sind, werden gemeldet und übersprungen; Dateien mit mehreren Stimmen stehen unter *Noch aufzuteilen*.

## Instrumente

*Instrumente* ist die eigene Liste Ihrer Einrichtung, nach Familien gruppiert: umbenennen, hinzufügen (mit Stimmung –
B, Es, F … – und weiteren Namen, z. B. „Alto Sax, A-Sax“), gruppieren und ordnen. Sie beginnt als Kopie der
eingebauten Liste von notenwart. Verwendete Instrumente lassen sich nicht löschen; **führen** Sie ein doppeltes
stattdessen mit dem Instrument **zusammen**, das bleibt – Stimmen, Mitglieder und Aufnahmen ziehen um, seine Namen
werden weitere Namen des bleibenden.

## Papierarchiv und Ausleihen

Den *Standort im Archiv* eines Werks („Schrank 3, Fach B“) setzen Sie unter *Details bearbeiten*. Gedruckte Stimmen
verleihen Sie im Abschnitt *Papierarchiv* des Werks: was („Kompletter Satz“ oder eine Stimme), wie viele Exemplare, an
wen – eine Person der Einrichtung oder jemand anderes mit Namen – und bis wann; dann *Ausleihen*.

*Ausleihen* (in der Bibliothek) zeigt alles, was draußen ist, Überfälliges markiert; *Als zurückgegeben vermerken*
nimmt eines oder mehrere zurück. Wer etwas ausgeliehen hat, sieht es auf *Meine Noten* und wird am Tag davor und
einmal bei Überfälligkeit erinnert.
