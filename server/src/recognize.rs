// SPDX-License-Identifier: AGPL-3.0-or-later

//! Reading what a sheet music PDF says about itself: title, composer, arranger, lyricist, publisher and the part
//! ("Trompete 2 in B") from the first page. Digital PDFs give their text layer (`pdftotext`), scans go through OCR
//! (`pdftoppm` + `tesseract`); rules turn the positioned lines into fields. An external AI, if the operator sets one
//! up (`ai.rs`), looks at the page image as well and also reads clef, key and time signature. Everything is a
//! suggestion for a person to check, never written on its own.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;

use serde::Serialize;
use tokio::process::Command;
use tokio::sync::Semaphore;
use uuid::Uuid;

use crate::ai::{Ai, AiInput};

/// A line of text on the page; coordinates are fractions of the page (0–1, y from the top).
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub text: String,
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

impl Line {
    fn height(&self) -> f32 {
        self.y1 - self.y0
    }
}

/// An instrument of the organisation's catalogue, for recognising the part.
#[derive(Debug, Clone)]
pub struct Known {
    pub id: Uuid,
    pub name: String,
    pub aliases: Vec<String>,
}

#[derive(Debug, Default, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Fields {
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub composer: Option<String>,
    pub arranger: Option<String>,
    pub lyricist: Option<String>,
    pub publisher: Option<String>,
    /// The part as printed, e.g. "2. Trompete in B".
    pub part: Option<String>,
    pub instrument_id: Option<Uuid>,
    pub voice: Option<i16>,
    /// Only an AI reads the music itself.
    pub clef: Option<String>,
    pub key: Option<String>,
    pub time_signature: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Recognition {
    /// Where the text came from: `text` (the PDF's own), `ocr`, or `none`.
    pub method: &'static str,
    /// The external AI added to the result.
    pub ai: bool,
    #[serde(flatten)]
    pub fields: Fields,
}

/// What this server can do, found out on start.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Abilities {
    pub text: bool,
    pub ocr: bool,
    pub ai: bool,
}

pub struct Recognizer {
    pub abilities: Abilities,
    /// Tesseract languages, e.g. `deu+eng` (only those installed).
    languages: String,
    ai: Option<Ai>,
    /// OCR and AI calls are slow and heavy; a few at a time.
    permits: Semaphore,
}

async fn runs(program: &str, arg: &str) -> bool {
    Command::new(program)
        .arg(arg)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .is_ok_and(|s| s.success())
}

impl Recognizer {
    /// Looks for the tools; `languages` are the wished Tesseract languages (`deu+eng`).
    pub async fn new(languages: &str, ai: Option<Ai>) -> Arc<Self> {
        let text = runs("pdftotext", "-v").await;
        let render = runs("pdftoppm", "-v").await;
        let installed: Vec<String> = match Command::new("tesseract").arg("--list-langs").output().await {
            Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout)
                .lines()
                .skip(1)
                .map(|l| l.trim().to_string())
                .collect(),
            _ => vec![],
        };
        let wanted: Vec<&str> = languages
            .split('+')
            .filter(|l| installed.iter().any(|i| i == l))
            .collect();
        let ocr = render && !wanted.is_empty();
        if !text {
            tracing::warn!("pdftotext is missing: text in PDFs is not recognised (install poppler-utils)");
        }
        if !ocr {
            tracing::warn!("pdftoppm or tesseract ({languages}) is missing: scans are not recognised");
        }
        Arc::new(Self {
            abilities: Abilities {
                text,
                ocr,
                ai: ai.is_some(),
            },
            languages: wanted.join("+"),
            ai,
            permits: Semaphore::new(2),
        })
    }

    /// The external AI, described for the system page ("Anthropic claude-opus-5-5 at …").
    pub fn ai_description(&self) -> Option<String> {
        self.ai.as_ref().map(|ai| ai.describe())
    }

    /// A recogniser that does nothing (tests, servers without the tools).
    pub fn disabled() -> Arc<Self> {
        Arc::new(Self {
            abilities: Abilities {
                text: false,
                ocr: false,
                ai: false,
            },
            languages: String::new(),
            ai: None,
            permits: Semaphore::new(1),
        })
    }

    /// Reads the first page of `pdf`.
    pub async fn recognize(&self, pdf: &[u8], instruments: &[Known]) -> anyhow::Result<Recognition> {
        let _permit = self.permits.acquire().await?;
        let dir = std::env::temp_dir().join(format!("notenwart-{}", Uuid::new_v4()));
        tokio::fs::create_dir_all(&dir).await?;
        let result = self.recognize_in(&dir, pdf, instruments).await;
        // Our own scratch files.
        let _ = tokio::fs::remove_dir_all(&dir).await;
        result
    }

    async fn recognize_in(&self, dir: &Path, pdf: &[u8], instruments: &[Known]) -> anyhow::Result<Recognition> {
        let file = dir.join("in.pdf");
        tokio::fs::write(&file, pdf).await?;
        let mut method = "none";
        let mut lines = vec![];
        if self.abilities.text {
            lines = text_layer(&file).await?;
            if words(&lines) >= 3 {
                method = "text";
            }
        }
        let mut image = None;
        if (method == "none" || self.ai.is_some()) && self.abilities.ocr {
            image = Some(render(&file, dir, if method == "none" { 300 } else { 150 }).await?);
        }
        if method == "none"
            && let Some(png) = &image
        {
            lines = ocr(png, &self.languages).await?;
            if words(&lines) >= 1 {
                method = "ocr";
            }
        }
        let mut fields = interpret(&lines, instruments);
        let mut ai = false;
        if let Some(client) = &self.ai {
            let png = match image {
                Some(path) => Some(tokio::fs::read(path).await?),
                None => None,
            };
            let input = AiInput {
                lines: &lines,
                image: png.as_deref(),
                instruments,
            };
            match client.read(input).await {
                Ok(found) => {
                    fields = merge(fields, found, instruments);
                    ai = true;
                }
                Err(error) => tracing::warn!("the AI could not read the page: {error:#}"),
            }
        }
        Ok(Recognition { method, ai, fields })
    }
}

fn words(lines: &[Line]) -> usize {
    lines.iter().map(|l| l.text.split_whitespace().count()).sum()
}

// ---- the parts in a PDF with all parts ----

/// A run of pages that belong to one part, read from the page headers.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PartProposal {
    /// The header as printed on its first page ("2. Trompete in B"); none for pages before the first header.
    pub printed: Option<String>,
    pub instrument_id: Option<Uuid>,
    pub voice: Option<i16>,
    /// A score or similar ("Partitur", "Direktion") rather than one instrument.
    pub score: bool,
    /// 1-based.
    pub pages: Vec<u32>,
}

/// The top of a page that names its part.
const HEADER: f32 = 0.2;
/// At most this many pages are read for one proposal.
pub const MAX_SPLIT_PAGES: usize = 400;

/// What the header of one page says: an instrument (with number), a score, or nothing.
#[derive(Debug, Clone, PartialEq)]
pub enum Header {
    Part {
        printed: String,
        instrument_id: Uuid,
        voice: Option<i16>,
    },
    Score {
        printed: String,
    },
    None,
}

/// Reads the header region of one page.
pub fn header(lines: &[Line], instruments: &[Known]) -> Header {
    let mut candidates: Vec<&Line> = lines
        .iter()
        .filter(|l| l.y0 < HEADER && l.text.split_whitespace().count() <= 6 && !labelled(&l.text))
        .collect();
    candidates.sort_by(|a, b| (a.x0 + a.y0).total_cmp(&(b.x0 + b.y0)));
    for line in &candidates {
        if let Some((id, voice)) = find_part(&line.text, instruments) {
            let voice = voice.or_else(|| {
                line.text
                    .split_whitespace()
                    .find_map(|w| w.ends_with('.').then(|| voice_number(w)).flatten())
            });
            return Header::Part {
                printed: line.text.trim().to_string(),
                instrument_id: id,
                voice,
            };
        }
    }
    candidates
        .iter()
        .find(|l| {
            let lower = l.text.to_lowercase();
            ["partitur", "score", "direktion", "dirigent", "conductor"]
                .iter()
                .any(|w| lower.split(|c: char| !c.is_alphanumeric()).any(|x| x == *w))
        })
        .map_or(Header::None, |l| Header::Score {
            printed: l.text.trim().to_string(),
        })
}

/// Groups pages by their headers: a page with a header starts a part unless it repeats the one before (headers on
/// continuation pages); pages without one continue the current part. Pages before the first header form a group
/// of their own (cover, notes).
pub fn group(headers: &[Header]) -> Vec<PartProposal> {
    let mut groups: Vec<PartProposal> = vec![];
    for (index, header) in headers.iter().enumerate() {
        let page = index as u32 + 1;
        let (printed, instrument_id, voice, score) = match header {
            Header::Part {
                printed,
                instrument_id,
                voice,
            } => (Some(printed.clone()), Some(*instrument_id), *voice, false),
            Header::Score { printed } => (Some(printed.clone()), None, None, true),
            Header::None => {
                match groups.last_mut() {
                    Some(last) => last.pages.push(page),
                    None => groups.push(PartProposal {
                        printed: None,
                        instrument_id: None,
                        voice: None,
                        score: false,
                        pages: vec![page],
                    }),
                }
                continue;
            }
        };
        if let Some(last) = groups.last_mut()
            && last.printed.is_some()
            && last.instrument_id == instrument_id
            && last.score == score
            && (last.voice == voice || voice.is_none())
        {
            last.pages.push(page);
            continue;
        }
        groups.push(PartProposal {
            printed,
            instrument_id,
            voice,
            score,
            pages: vec![page],
        });
    }
    groups
}

impl Recognizer {
    /// Proposes the parts in a PDF that holds several, from the header of every page.
    pub async fn split(&self, pdf: &[u8], instruments: &[Known]) -> anyhow::Result<Vec<PartProposal>> {
        let _permit = self.permits.acquire().await?;
        let dir = std::env::temp_dir().join(format!("notenwart-{}", Uuid::new_v4()));
        tokio::fs::create_dir_all(&dir).await?;
        let result = self.split_in(&dir, pdf, instruments).await;
        let _ = tokio::fs::remove_dir_all(&dir).await;
        result
    }

    async fn split_in(&self, dir: &Path, pdf: &[u8], instruments: &[Known]) -> anyhow::Result<Vec<PartProposal>> {
        let file = dir.join("in.pdf");
        tokio::fs::write(&file, pdf).await?;
        let out = Command::new("pdftotext")
            .args(["-l", &MAX_SPLIT_PAGES.to_string(), "-bbox-layout"])
            .arg(&file)
            .arg("-")
            .output()
            .await?;
        anyhow::ensure!(out.status.success(), "pdftotext cannot read the PDF");
        let pages = parse_pages(&String::from_utf8_lossy(&out.stdout));
        let mut headers = Vec::with_capacity(pages.len());
        for (index, page) in pages.iter().enumerate() {
            let mut lines: Vec<Line> = page.lines.iter().filter(|l| l.y0 < HEADER).cloned().collect();
            // A scanned page: read only its top strip.
            if words(&page.lines) < 3 && self.abilities.ocr {
                lines = ocr_header(&file, dir, index as u32 + 1, page, &self.languages).await?;
            }
            headers.push(header(&lines, instruments));
        }
        Ok(group(&headers))
    }
}

async fn ocr_header(pdf: &Path, dir: &Path, page: u32, size: &Page, languages: &str) -> anyhow::Result<Vec<Line>> {
    const DPI: f32 = 250.0;
    let width = (size.width * DPI / 72.0).round() as u32;
    let height = (size.height * HEADER * DPI / 72.0).round() as u32;
    let base = dir.join(format!("header{page}"));
    let status = Command::new("pdftoppm")
        .args([
            "-f",
            &page.to_string(),
            "-l",
            &page.to_string(),
            "-singlefile",
            "-gray",
            "-png",
        ])
        .args([
            "-r",
            &DPI.to_string(),
            "-x",
            "0",
            "-y",
            "0",
            "-W",
            &width.to_string(),
            "-H",
            &height.to_string(),
        ])
        .arg(pdf)
        .arg(&base)
        .stderr(Stdio::null())
        .status()
        .await?;
    anyhow::ensure!(status.success(), "pdftoppm failed");
    let png = base.with_extension("png");
    let lines = ocr(&png, languages).await?;
    let _ = tokio::fs::remove_file(&png).await;
    // Coordinates of the strip → of the page.
    Ok(lines
        .into_iter()
        .map(|l| Line {
            y0: l.y0 * HEADER,
            y1: l.y1 * HEADER,
            ..l
        })
        .collect())
}

// ---- getting positioned lines ----

async fn text_layer(pdf: &Path) -> anyhow::Result<Vec<Line>> {
    let out = Command::new("pdftotext")
        .args(["-f", "1", "-l", "1", "-bbox-layout"])
        .arg(pdf)
        .arg("-")
        .output()
        .await?;
    if !out.status.success() {
        // Broken or encrypted PDFs: nothing to read.
        return Ok(vec![]);
    }
    Ok(parse_bbox(&String::from_utf8_lossy(&out.stdout)))
}

fn attribute(tag: &str, name: &str) -> Option<f32> {
    let start = tag.find(&format!("{name}=\""))? + name.len() + 2;
    let end = tag[start..].find('"')? + start;
    tag[start..end].parse().ok()
}

fn unescape(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
}

/// A page of `pdftotext -bbox-layout` output: its size in points and its lines.
#[derive(Debug, Clone, PartialEq)]
pub struct Page {
    pub width: f32,
    pub height: f32,
    pub lines: Vec<Line>,
}

/// Lines of the first page of `pdftotext -bbox-layout` output.
pub fn parse_bbox(xhtml: &str) -> Vec<Line> {
    parse_pages(xhtml)
        .into_iter()
        .next()
        .map(|p| p.lines)
        .unwrap_or_default()
}

/// Every page of `pdftotext -bbox-layout` output (XHTML with page, line and word boxes in points).
pub fn parse_pages(xhtml: &str) -> Vec<Page> {
    let mut pages = vec![];
    let mut rest = xhtml;
    while let Some(start) = rest.find("<page ") {
        let tag_end = rest[start..].find('>').map_or(rest.len(), |e| start + e);
        let end = rest[tag_end..].find("</page>").map_or(rest.len(), |e| tag_end + e);
        let tag = &rest[start..tag_end];
        if let (Some(width), Some(height)) = (attribute(tag, "width"), attribute(tag, "height")) {
            pages.push(Page {
                width,
                height,
                lines: page_lines(&rest[tag_end..end], width, height),
            });
        }
        rest = &rest[end..];
        if rest.starts_with("</page>") {
            rest = &rest[7..];
        }
    }
    pages
}

fn page_lines(xhtml: &str, width: f32, height: f32) -> Vec<Line> {
    let mut lines = vec![];
    let mut rest = xhtml;
    while let Some(start) = rest.find("<line ") {
        let Some(end) = rest[start..].find("</line>").map(|e| start + e) else {
            break;
        };
        let block = &rest[start..end];
        let tag = &block[..block.find('>').unwrap_or(block.len())];
        let mut text = vec![];
        let mut words = block;
        while let Some(w) = words.find("<word ") {
            let open = words[w..].find('>').map_or(words.len(), |e| w + e + 1);
            let close = words[open..].find("</word>").map_or(words.len(), |e| open + e);
            text.push(unescape(&words[open..close]));
            words = &words[close..];
        }
        if let (Some(x0), Some(y0), Some(x1), Some(y1)) = (
            attribute(tag, "xMin"),
            attribute(tag, "yMin"),
            attribute(tag, "xMax"),
            attribute(tag, "yMax"),
        ) {
            lines.push(Line {
                text: text.join(" "),
                x0: x0 / width,
                y0: y0 / height,
                x1: x1 / width,
                y1: y1 / height,
            });
        }
        rest = &rest[end + 7..];
    }
    lines
}

async fn render(pdf: &Path, dir: &Path, dpi: u32) -> anyhow::Result<PathBuf> {
    let base = dir.join(format!("page{dpi}"));
    let status = Command::new("pdftoppm")
        .args([
            "-f",
            "1",
            "-l",
            "1",
            "-singlefile",
            "-gray",
            "-png",
            "-r",
            &dpi.to_string(),
        ])
        .arg(pdf)
        .arg(&base)
        .stderr(Stdio::null())
        .status()
        .await?;
    anyhow::ensure!(status.success(), "pdftoppm failed");
    Ok(base.with_extension("png"))
}

async fn ocr(png: &Path, languages: &str) -> anyhow::Result<Vec<Line>> {
    let out = Command::new("tesseract")
        .arg(png)
        .arg("-")
        .args(["-l", languages, "--psm", "11", "tsv"])
        .stderr(Stdio::null())
        .output()
        .await?;
    anyhow::ensure!(out.status.success(), "tesseract failed");
    Ok(parse_tsv(&String::from_utf8_lossy(&out.stdout)))
}

/// Lines from Tesseract's TSV output: words (level 5) grouped by block, paragraph and line; unsure words dropped.
pub fn parse_tsv(tsv: &str) -> Vec<Line> {
    let mut size = (1.0f32, 1.0f32);
    let mut lines: Vec<((u32, u32, u32), Line)> = vec![];
    for row in tsv.lines().skip(1) {
        let cols: Vec<&str> = row.split('\t').collect();
        if cols.len() < 12 {
            continue;
        }
        let n = |i: usize| cols[i].parse::<f32>().unwrap_or(0.0);
        match cols[0] {
            "1" => size = (n(8).max(1.0), n(9).max(1.0)),
            "5" => {
                let text = cols[11].trim();
                // Staves, notes and dust come out as short garbage with low confidence.
                if text.is_empty() || n(10) < 55.0 {
                    continue;
                }
                let key = (n(2) as u32, n(3) as u32, n(4) as u32);
                let (x0, y0) = (n(6) / size.0, n(7) / size.1);
                let (x1, y1) = ((n(6) + n(8)) / size.0, (n(7) + n(9)) / size.1);
                match lines.iter_mut().find(|(k, _)| *k == key) {
                    Some((_, line)) => {
                        line.text.push(' ');
                        line.text.push_str(text);
                        line.x0 = line.x0.min(x0);
                        line.y0 = line.y0.min(y0);
                        line.x1 = line.x1.max(x1);
                        line.y1 = line.y1.max(y1);
                    }
                    None => lines.push((
                        key,
                        Line {
                            text: text.to_string(),
                            x0,
                            y0,
                            x1,
                            y1,
                        },
                    )),
                }
            }
            _ => {}
        }
    }
    let mut lines: Vec<Line> = lines.into_iter().map(|(_, l)| l).collect();
    lines.sort_by(|a, b| a.y0.total_cmp(&b.y0).then(a.x0.total_cmp(&b.x0)));
    lines
}

// ---- rules ----

const TOP: f32 = 0.33;
const BOTTOM: f32 = 0.85;

/// Labels before a name, lower case, longest first where one starts another.
const COMPOSER: &[&str] = &[
    "musik und text:",
    "music and lyrics:",
    "komposition:",
    "komponist:",
    "composer:",
    "music by",
    "melodie:",
    "musik:",
    "music:",
    "comp.:",
    "comp.",
    "komp.:",
    "komp.",
    "weise:",
];
const ARRANGER: &[&str] = &[
    "arrangement:",
    "arranged by",
    "arrangiert von",
    "arrangiert:",
    "bearbeitung:",
    "bearbeitet von",
    "instrumentation:",
    "orchestration:",
    "satz:",
    "arr.:",
    "arr.",
    "arr:",
    "bearb.:",
    "bearb.",
];
const LYRICIST: &[&str] = &["words and music:", "lyrics:", "words:", "worte:", "text:", "dichtung:"];

fn after_label(text: &str, labels: &[&str]) -> Option<String> {
    let lower = text.to_lowercase();
    for label in labels {
        if let Some(i) = lower.find(label) {
            // Labels start a line or follow a separator ("Musik: X / Arr.: Y").
            let before = lower[..i].trim_end();
            if !(before.is_empty() || before.ends_with(['/', '|', ',', ';', '–', '-'])) {
                continue;
            }
            let rest = &text[i + label.len()..];
            let rest = cut_at_other_label(rest);
            let name = clean_name(rest);
            if !name.is_empty() {
                return Some(name);
            }
        }
    }
    None
}

/// "Ernst Mosch / Arr.: Kurt Gäble" → "Ernst Mosch ".
fn cut_at_other_label(text: &str) -> &str {
    let lower = text.to_lowercase();
    let mut end = text.len();
    for label in COMPOSER.iter().chain(ARRANGER).chain(LYRICIST) {
        if let Some(i) = lower.find(label)
            && i > 0
        {
            end = end.min(i);
        }
    }
    let cut = &text[..end];
    cut.trim_end_matches(|c: char| c.is_whitespace() || "/|,;–-".contains(c))
}

/// Trims separators, life dates "(1770–1827)" and stray punctuation.
fn clean_name(text: &str) -> String {
    let mut name = text.trim().trim_start_matches([':', '-', '–']).trim().to_string();
    if let Some(open) = name.find('(')
        && name[open..].chars().filter(char::is_ascii_digit).count() >= 4
    {
        name.truncate(open);
    }
    name.trim_matches(|c: char| c.is_whitespace() || ",;/|".contains(c))
        .to_string()
}

fn publisher(text: &str) -> Option<String> {
    let lower = text.to_lowercase();
    let marked = text.contains('©')
        || lower.starts_with("(c)")
        || lower.contains("copyright")
        || [
            "verlag",
            "edition",
            "editions",
            "publishing",
            "musikverlag",
            "music publisher",
            "verlags",
        ]
        .iter()
        .any(|w| {
            lower
                .split(|c: char| !c.is_alphanumeric())
                .any(|x| x == *w || x.ends_with("verlag"))
        });
    if !marked {
        return None;
    }
    let mut name = text.replace('©', " ");
    for prefix in ["copyright by", "copyright", "(c)", "(C)", "by"] {
        if name.trim_start().to_lowercase().starts_with(prefix) {
            name = name.trim_start()[prefix.len()..].to_string();
        }
    }
    // A leading year "2019 by" or "© 1998".
    let name = name
        .trim()
        .trim_start_matches(|c: char| c.is_ascii_digit() || c == ' ' || c == '-' || c == '–')
        .trim();
    let name = name.strip_prefix("by ").unwrap_or(name);
    // "All rights reserved" and the like.
    let name = name.split(['.', '·']).next().unwrap_or(name);
    let name = clean_name(name);
    (name.chars().filter(|c| c.is_alphabetic()).count() >= 3).then_some(name)
}

fn normalize(text: &str) -> String {
    let words: Vec<String> = text
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect();
    format!(" {} ", words.join(" "))
}

fn roman(word: &str) -> Option<i16> {
    match word {
        "i" => Some(1),
        "ii" => Some(2),
        "iii" => Some(3),
        "iv" => Some(4),
        "v" => Some(5),
        "vi" => Some(6),
        _ => None,
    }
}

fn voice_number(word: &str) -> Option<i16> {
    let digits = word.trim_end_matches(['.', ')']);
    let digits = digits
        .strip_suffix("st")
        .or_else(|| digits.strip_suffix("nd"))
        .or_else(|| digits.strip_suffix("rd"))
        .or_else(|| digits.strip_suffix("th"))
        .unwrap_or(digits);
    match digits.parse::<i16>() {
        Ok(n) if (1..=12).contains(&n) => Some(n),
        _ => roman(digits),
    }
}

/// The instrument a line names as a whole word, with the part number before or after it ("2. Trompete",
/// "Trompete 2", "Clarinet II"). The longest name wins.
pub fn find_part(text: &str, instruments: &[Known]) -> Option<(Uuid, Option<i16>)> {
    let haystack = normalize(text);
    let mut best: Option<(usize, usize, &Known)> = None;
    for instrument in instruments {
        for name in std::iter::once(&instrument.name).chain(&instrument.aliases) {
            let needle = normalize(name);
            if needle.trim().len() < 2 {
                continue;
            }
            if let Some(at) = haystack.find(&needle)
                && best.is_none_or(|(len, _, _)| needle.len() > len)
            {
                best = Some((needle.len(), at, instrument));
            }
        }
    }
    let (len, at, instrument) = best?;
    let after = haystack[at + len..].split_whitespace().next();
    let before = haystack[..at].split_whitespace().last();
    // The original text keeps "2." which normalize drops, so look for numbers there too.
    let voice = after.and_then(voice_number).or_else(|| before.and_then(voice_number));
    Some((instrument.id, voice))
}

fn labelled(text: &str) -> bool {
    after_label(text, COMPOSER).is_some()
        || after_label(text, ARRANGER).is_some()
        || after_label(text, LYRICIST).is_some()
}

/// Turns the lines of a first page into fields.
pub fn interpret(lines: &[Line], instruments: &[Known]) -> Fields {
    let mut fields = Fields::default();
    let lines: Vec<&Line> = lines
        .iter()
        .filter(|l| {
            let letters = l.text.chars().filter(|c| c.is_alphabetic()).count();
            letters >= 2 && l.height() > 0.0
        })
        .collect();

    for line in &lines {
        let text = line.text.trim();
        if fields.composer.is_none() {
            fields.composer = after_label(text, COMPOSER);
        }
        if fields.arranger.is_none() {
            fields.arranger = after_label(text, ARRANGER);
        }
        if fields.lyricist.is_none() {
            fields.lyricist = after_label(text, LYRICIST);
        }
        if fields.publisher.is_none() && (line.y0 > BOTTOM || line.y1 < 0.12) {
            fields.publisher = publisher(text);
        }
    }

    let top: Vec<&Line> = lines.iter().copied().filter(|l| l.y0 < TOP).collect();

    // The part: a short line near the top that names an instrument; top left first.
    let mut parts: Vec<(&Line, (Uuid, Option<i16>))> = top
        .iter()
        .filter(|l| l.text.split_whitespace().count() <= 6 && !labelled(&l.text))
        .filter_map(|l| find_part(&l.text, instruments).map(|p| (*l, p)))
        .collect();
    parts.sort_by(|a, b| (a.0.x0 + a.0.y0).total_cmp(&(b.0.x0 + b.0.y0)));
    if let Some((line, (id, voice))) = parts.first() {
        fields.part = Some(line.text.trim().to_string());
        fields.instrument_id = Some(*id);
        fields.voice = voice.or_else(|| {
            // "2. Trompete" – the number with its dot.
            line.text
                .split_whitespace()
                .find_map(|w| w.ends_with('.').then(|| voice_number(w)).flatten())
        });
    }
    let part_line = parts.first().map(|(l, _)| *l);

    // The title: the largest text near the top that is neither a label nor the part.
    let candidates: Vec<&Line> = top
        .iter()
        .copied()
        .filter(|l| Some(*l) != part_line && !labelled(&l.text) && publisher(&l.text).is_none())
        .collect();
    if let Some(title) = candidates
        .iter()
        .max_by(|a, b| a.height().total_cmp(&b.height()).then(b.y0.total_cmp(&a.y0)))
    {
        let heights: Vec<f32> = lines.iter().map(|l| l.height()).collect();
        let median = median(&heights);
        if title.height() >= median * 1.15 || lines.len() <= 3 {
            fields.title = Some(clean_name(&title.text));
            // A subtitle right below, smaller but not tiny, roughly centred under it.
            fields.subtitle = candidates
                .iter()
                .filter(|l| l.y0 > title.y0 && l.y0 - title.y1 < title.height() * 1.5)
                .filter(|l| l.height() < title.height() * 0.95 && l.height() > title.height() * 0.35)
                .filter(|l| ((l.x0 + l.x1) / 2.0 - (title.x0 + title.x1) / 2.0).abs() < 0.15)
                .min_by(|a, b| a.y0.total_cmp(&b.y0))
                .map(|l| clean_name(&l.text));
        }
    }

    // An unlabelled composer: a name set flush right near the top.
    if fields.composer.is_none() {
        fields.composer = candidates
            .iter()
            .filter(|l| l.x1 > 0.75 && l.x0 > 0.5)
            .filter(|l| Some(clean_name(&l.text)) != fields.title)
            .filter(|l| {
                let words = l.text.split_whitespace().count();
                (1..=5).contains(&words) && l.text.chars().filter(|c| c.is_ascii_digit()).count() <= 9
            })
            .min_by(|a, b| a.y0.total_cmp(&b.y0))
            .map(|l| clean_name(&l.text))
            .filter(|n| n.chars().any(char::is_alphabetic));
    }
    fields
}

fn median(values: &[f32]) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f32::total_cmp);
    sorted[sorted.len() / 2]
}

/// The AI's reading wins where it found something; the instrument is mapped to the catalogue.
pub fn merge(rules: Fields, ai: Fields, instruments: &[Known]) -> Fields {
    let pick = |a: Option<String>, b: Option<String>| a.filter(|s| !s.trim().is_empty()).or(b);
    let from_ai_part = ai.part.as_deref().and_then(|p| find_part(p, instruments));
    Fields {
        title: pick(ai.title, rules.title),
        subtitle: pick(ai.subtitle, rules.subtitle),
        composer: pick(ai.composer, rules.composer),
        arranger: pick(ai.arranger, rules.arranger),
        lyricist: pick(ai.lyricist, rules.lyricist),
        publisher: pick(ai.publisher, rules.publisher),
        instrument_id: ai.instrument_id.or(from_ai_part.map(|p| p.0)).or(rules.instrument_id),
        voice: ai.voice.or(from_ai_part.and_then(|p| p.1)).or(rules.voice),
        part: pick(ai.part, rules.part),
        clef: pick(ai.clef, rules.clef),
        key: pick(ai.key, rules.key),
        time_signature: pick(ai.time_signature, rules.time_signature),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(text: &str, x0: f32, y0: f32, x1: f32, y1: f32) -> Line {
        Line {
            text: text.into(),
            x0,
            y0,
            x1,
            y1,
        }
    }

    fn catalogue() -> Vec<Known> {
        let known = |name: &str, aliases: &[&str]| Known {
            id: Uuid::new_v4(),
            name: name.into(),
            aliases: aliases.iter().map(|a| a.to_string()).collect(),
        };
        vec![
            known("Trompete", &["Trumpet", "Trp"]),
            known("Klarinette", &["Clarinet", "Klar"]),
            known("Bassklarinette", &["Bass clarinet"]),
            known("Posaune", &["Trombone"]),
        ]
    }

    #[test]
    fn reads_a_typical_first_page() {
        let instruments = catalogue();
        let lines = vec![
            line("2. Trompete in B", 0.08, 0.05, 0.3, 0.07),
            line("Böhmischer Traum", 0.3, 0.08, 0.7, 0.13),
            line("Polka", 0.45, 0.14, 0.55, 0.16),
            line("Musik: Norbert Gälle", 0.65, 0.17, 0.92, 0.19),
            line("Arr.: Kurt Gäble", 0.7, 0.19, 0.92, 0.21),
            line("12", 0.1, 0.4, 0.12, 0.41),
            line(
                "© 1998 by Musikverlag Rundel GmbH. All rights reserved.",
                0.2,
                0.95,
                0.8,
                0.965,
            ),
        ];
        let fields = interpret(&lines, &instruments);
        assert_eq!(fields.title.as_deref(), Some("Böhmischer Traum"));
        assert_eq!(fields.subtitle.as_deref(), Some("Polka"));
        assert_eq!(fields.composer.as_deref(), Some("Norbert Gälle"));
        assert_eq!(fields.arranger.as_deref(), Some("Kurt Gäble"));
        assert_eq!(fields.publisher.as_deref(), Some("Musikverlag Rundel GmbH"));
        assert_eq!(fields.instrument_id, Some(instruments[0].id));
        assert_eq!(fields.voice, Some(2));
        assert_eq!(fields.part.as_deref(), Some("2. Trompete in B"));
    }

    #[test]
    fn finds_unlabelled_composers_and_combined_labels() {
        let instruments = catalogue();
        let lines = vec![
            line("Clarinet II", 0.05, 0.04, 0.2, 0.06),
            line("Ode an die Freude", 0.25, 0.08, 0.75, 0.12),
            line("Ludwig van Beethoven (1770–1827)", 0.6, 0.14, 0.95, 0.16),
        ];
        let fields = interpret(&lines, &instruments);
        assert_eq!(fields.composer.as_deref(), Some("Ludwig van Beethoven"));
        assert_eq!(fields.instrument_id, Some(instruments[1].id));
        assert_eq!(fields.voice, Some(2));

        assert_eq!(
            after_label("Musik: Ernst Mosch / Arr.: Kurt Gäble", COMPOSER).as_deref(),
            Some("Ernst Mosch")
        );
        assert_eq!(
            after_label("Musik: Ernst Mosch / Arr.: Kurt Gäble", ARRANGER).as_deref(),
            Some("Kurt Gäble")
        );
        assert_eq!(
            after_label("Text: Joseph Mohr", LYRICIST).as_deref(),
            Some("Joseph Mohr")
        );
        // Words that merely contain a label are no labels.
        assert_eq!(after_label("Kontext: nichts", LYRICIST), None);
    }

    #[test]
    fn the_longest_instrument_name_wins() {
        let instruments = catalogue();
        assert_eq!(
            find_part("Bass Clarinet 1", &instruments),
            Some((instruments[2].id, Some(1)))
        );
        assert_eq!(
            find_part("1st Trumpet", &instruments),
            Some((instruments[0].id, Some(1)))
        );
        assert_eq!(find_part("Partitur", &instruments), None);
    }

    #[test]
    fn parses_pdftotext_and_tesseract_output() {
        let xhtml = r#"<doc><page width="600.000000" height="800.000000"><flow><block>
            <line xMin="60" yMin="80" xMax="300" yMax="120"><word xMin="60" yMin="80" xMax="150" yMax="120">Rock</word>
            <word xMin="160" yMin="80" xMax="300" yMax="120">&amp; Roll</word></line></block></flow></page></doc>"#;
        assert_eq!(parse_bbox(xhtml), vec![line("Rock & Roll", 0.1, 0.1, 0.5, 0.15)]);

        let tsv = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n\
                   1\t1\t0\t0\t0\t0\t0\t0\t1000\t2000\t-1\t\n\
                   5\t1\t1\t1\t1\t1\t100\t200\t200\t100\t91\tStille\n\
                   5\t1\t1\t1\t1\t2\t320\t200\t180\t100\t89\tNacht\n\
                   5\t1\t2\t1\t1\t1\t500\t900\t20\t20\t12\t~\n";
        assert_eq!(parse_tsv(tsv), vec![line("Stille Nacht", 0.1, 0.1, 0.5, 0.15)]);
    }

    #[test]
    fn pages_are_grouped_by_their_headers() {
        let instruments = catalogue();
        let page = |text: &str| vec![line(text, 0.05, 0.03, 0.3, 0.05), line("Allegro", 0.4, 0.3, 0.5, 0.32)];
        let headers: Vec<Header> = [
            page("Ohne Kopf"),
            page("Partitur"),
            page("Partitur"),
            page("1. Trompete in B"),
            page("Ohne Kopf"),
            page("2. Trompete in B"),
            page("Trompete 2"),
            page("Klarinette"),
        ]
        .iter()
        .map(|lines| header(lines, &instruments))
        .collect();
        let groups = group(&headers);
        type Summary = (Option<Uuid>, Option<i16>, bool, Vec<u32>);
        let summary: Vec<Summary> = groups
            .iter()
            .map(|g| (g.instrument_id, g.voice, g.score, g.pages.clone()))
            .collect();
        let (trumpet, clarinet) = (instruments[0].id, instruments[1].id);
        assert_eq!(
            summary,
            vec![
                (None, None, false, vec![1]),
                (None, None, true, vec![2, 3]),
                (Some(trumpet), Some(1), false, vec![4, 5]),
                (Some(trumpet), Some(2), false, vec![6, 7]),
                (Some(clarinet), None, false, vec![8]),
            ]
        );
        assert_eq!(groups[3].printed.as_deref(), Some("2. Trompete in B"));
    }

    #[test]
    fn reads_every_page_of_pdftotext_output() {
        let xhtml = r#"<doc><page width="100" height="200"><line xMin="10" yMin="20" xMax="50" yMax="30"><word xMin="10">A</word></line></page>
            <page width="100" height="200"></page><page width="100" height="200"><line xMin="0" yMin="0" xMax="10" yMax="10"><word xMin="0">C</word></line></page></doc>"#;
        let pages = parse_pages(xhtml);
        assert_eq!(pages.iter().map(|p| p.lines.len()).collect::<Vec<_>>(), vec![1, 0, 1]);
        assert_eq!(pages[2].lines[0].text, "C");
    }

    #[test]
    fn publishers_are_found_in_footers() {
        assert_eq!(
            publisher("© Copyright 2005 by Edition Helbling").as_deref(),
            Some("Edition Helbling")
        );
        assert_eq!(publisher("Seite 3"), None);
    }
}
