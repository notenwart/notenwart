// SPDX-License-Identifier: AGPL-3.0-or-later

//! Reading uploaded PDFs, extracting pages into a part without re-encoding anything, and drawing annotations into a
//! copy for printing.

use std::collections::BTreeMap;

use lopdf::{Dictionary, Document, Object, ObjectId, Stream, dictionary};

#[derive(Debug, thiserror::Error)]
pub enum PdfError {
    #[error("not a readable PDF: {0}")]
    Unreadable(String),
    #[error("the PDF has no pages")]
    NoPages,
    #[error("the PDF has no page {0}")]
    NoSuchPage(u32),
    #[error("cannot write the PDF: {0}")]
    Write(String),
}

/// Number of pages; fails for anything that is not a PDF with at least one page.
pub fn page_count(bytes: &[u8]) -> Result<u32, PdfError> {
    let document = load(bytes)?;
    match document.get_pages().len() {
        0 => Err(PdfError::NoPages),
        n => Ok(n as u32),
    }
}

/// A new PDF with the given 1-based pages of `bytes`, in the given order.
///
/// The kept pages get a fresh page tree; outlines, named destinations and forms of the source are dropped because
/// they would point to pages that are gone.
pub fn extract(bytes: &[u8], pages: &[u32]) -> Result<Vec<u8>, PdfError> {
    let mut document = load(bytes)?;
    // Objects are decrypted on load; write the result unencrypted.
    document.encryption_state = None;
    document.trailer.remove(b"Encrypt");

    let all = document.get_pages();
    let kids = pages
        .iter()
        .map(|number| all.get(number).copied().ok_or(PdfError::NoSuchPage(*number)))
        .collect::<Result<Vec<ObjectId>, _>>()?;

    // Attributes a page may inherit from its ancestors must move onto the page before it changes parents.
    for &page in &kids {
        for key in [&b"Resources"[..], b"MediaBox", b"CropBox", b"Rotate"] {
            let has_own = document.get_dictionary(page).map(|page| page.has(key)).unwrap_or(false);
            if !has_own && let Some(value) = inherited(&document, page, key) {
                set(&mut document, page, key, value)?;
            }
        }
    }

    let tree = document.new_object_id();
    for &page in &kids {
        set(&mut document, page, b"Parent", Object::Reference(tree))?;
    }
    document.objects.insert(
        tree,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => kids.iter().copied().map(Object::Reference).collect::<Vec<_>>(),
            "Count" => kids.len() as i64,
        }),
    );

    let catalog = document.catalog_mut().map_err(|e| PdfError::Unreadable(e.to_string()))?;
    catalog.set("Pages", Object::Reference(tree));
    for key in ["Outlines", "Names", "Dests", "PageLabels", "OpenAction", "AcroForm", "StructTreeRoot"] {
        catalog.remove(key.as_bytes());
    }

    document.prune_objects();
    let mut out = Vec::new();
    document.save_to(&mut out).map_err(|e| PdfError::Write(e.to_string()))?;
    Ok(out)
}

/// One stroke to draw: points as fractions of the page as shown (0–1 across, 0–1 down), width as a fraction of the
/// shown page's width.
#[derive(Debug)]
pub struct Mark {
    pub rgb: [f32; 3],
    pub width: f32,
    pub highlighter: bool,
    pub points: Vec<[f32; 2]>,
}

/// A copy of `bytes` with `marks` (by 1-based page) drawn on top, as vector paths. Pages keep their content; it is
/// wrapped in `q … Q` so the marks start from a clean graphics state.
pub fn stamp(bytes: &[u8], marks: &BTreeMap<u32, Vec<Mark>>) -> Result<Vec<u8>, PdfError> {
    let mut document = load(bytes)?;
    document.encryption_state = None;
    document.trailer.remove(b"Encrypt");
    let pages = document.get_pages();
    for (number, list) in marks {
        if list.is_empty() {
            continue;
        }
        let page = *pages.get(number).ok_or(PdfError::NoSuchPage(*number))?;
        let area = page_box(&document, page)?;
        let rotate = match own_or_inherited(&document, page, b"Rotate") {
            Some(Object::Integer(r)) => r.rem_euclid(360),
            _ => 0,
        };
        let content = marks_content(list, area, rotate);

        // Resources of its own (a copy of the inherited ones), with a transparent state for the highlighter.
        let mut resources = match own_or_inherited(&document, page, b"Resources") {
            Some(Object::Reference(id)) => document.get_dictionary(id).cloned().unwrap_or_default(),
            Some(Object::Dictionary(d)) => d,
            _ => Dictionary::new(),
        };
        let mut states = match resources.get(b"ExtGState") {
            Ok(Object::Reference(id)) => document.get_dictionary(*id).cloned().unwrap_or_default(),
            Ok(Object::Dictionary(d)) => d.clone(),
            _ => Dictionary::new(),
        };
        states.set("NwHl", dictionary! { "Type" => "ExtGState", "CA" => 0.35, "BM" => "Multiply" });
        resources.set("ExtGState", Object::Dictionary(states));
        set(&mut document, page, b"Resources", Object::Dictionary(resources))?;

        let before = document.add_object(Stream::new(dictionary! {}, b"q\n".to_vec()));
        let after = document.add_object(Stream::new(dictionary! {}, b"\nQ\n".to_vec()));
        let drawn = document.add_object(Stream::new(dictionary! {}, content.into_bytes()));
        let existing = match document.get_dictionary(page).ok().and_then(|p| p.get(b"Contents").ok()) {
            Some(Object::Array(parts)) => parts.clone(),
            Some(other) => vec![other.clone()],
            None => vec![],
        };
        let mut contents = vec![Object::Reference(before)];
        contents.extend(existing);
        contents.extend([Object::Reference(after), Object::Reference(drawn)]);
        set(&mut document, page, b"Contents", Object::Array(contents))?;
    }
    let mut out = Vec::new();
    document.save_to(&mut out).map_err(|e| PdfError::Write(e.to_string()))?;
    Ok(out)
}

/// The visible area: CropBox, else MediaBox, as [x0, y0, x1, y1].
fn page_box(document: &Document, page: ObjectId) -> Result<[f32; 4], PdfError> {
    for key in [&b"CropBox"[..], b"MediaBox"] {
        let value = match own_or_inherited(document, page, key) {
            Some(Object::Reference(id)) => document.get_object(id).ok().cloned(),
            other => other,
        };
        if let Some(Object::Array(values)) = value {
            let n: Vec<f32> = values
                .iter()
                .filter_map(|v| match v {
                    Object::Integer(i) => Some(*i as f32),
                    Object::Real(r) => Some(*r),
                    _ => None,
                })
                .collect();
            if n.len() == 4 {
                return Ok([n[0].min(n[2]), n[1].min(n[3]), n[0].max(n[2]), n[1].max(n[3])]);
            }
        }
    }
    Err(PdfError::Unreadable("a page without a size".into()))
}

/// The content stream drawing `marks` on a page with this box and rotation.
fn marks_content(marks: &[Mark], [x0, y0, x1, y1]: [f32; 4], rotate: i64) -> String {
    let (w, h) = (x1 - x0, y1 - y0);
    // From the shown page (u across, v down) to the page's own coordinates.
    let to_page = |[u, v]: [f32; 2]| -> (f32, f32) {
        match rotate {
            90 => (x0 + v * w, y0 + u * h),
            180 => (x1 - u * w, y0 + v * h),
            270 => (x1 - v * w, y1 - u * h),
            _ => (x0 + u * w, y1 - v * h),
        }
    };
    let shown_width = if rotate == 90 || rotate == 270 { h } else { w };
    let mut out = String::from("q 1 J 1 j\n");
    for mark in marks {
        let Some(&first) = mark.points.first() else { continue };
        out.push_str("q ");
        if mark.highlighter {
            out.push_str("/NwHl gs ");
        }
        let [r, g, b] = mark.rgb;
        out.push_str(&format!("{r:.3} {g:.3} {b:.3} RG {:.2} w\n", mark.width * shown_width));
        let (x, y) = to_page(first);
        out.push_str(&format!("{x:.2} {y:.2} m\n"));
        if mark.points.len() == 1 {
            // A dot: a line too short to see but long enough to get round caps.
            out.push_str(&format!("{:.2} {y:.2} l\n", x + 0.01));
        }
        for &point in &mark.points[1..] {
            let (x, y) = to_page(point);
            out.push_str(&format!("{x:.2} {y:.2} l\n"));
        }
        out.push_str("S Q\n");
    }
    out.push_str("Q\n");
    out
}

fn own_or_inherited(document: &Document, page: ObjectId, key: &[u8]) -> Option<Object> {
    match document.get_dictionary(page).ok()?.get(key) {
        Ok(value) => Some(value.clone()),
        Err(_) => inherited(document, page, key),
    }
}

fn load(bytes: &[u8]) -> Result<Document, PdfError> {
    if !bytes.starts_with(b"%PDF-") {
        return Err(PdfError::Unreadable("missing %PDF header".into()));
    }
    Document::load_mem(bytes).map_err(|e| PdfError::Unreadable(e.to_string()))
}

fn inherited(document: &Document, page: ObjectId, key: &[u8]) -> Option<Object> {
    let mut node = document.get_dictionary(page).ok()?;
    // Bounded walk: a broken file could contain a cycle.
    for _ in 0..64 {
        let parent = node.get(b"Parent").and_then(Object::as_reference).ok()?;
        node = document.get_dictionary(parent).ok()?;
        if let Ok(value) = node.get(key) {
            return Some(value.clone());
        }
    }
    None
}

fn set(document: &mut Document, id: ObjectId, key: &[u8], value: Object) -> Result<(), PdfError> {
    document.get_dictionary_mut(id).map_err(|e| PdfError::Unreadable(e.to_string()))?.set(key.to_vec(), value);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mark(points: Vec<[f32; 2]>) -> Mark {
        Mark { rgb: [1.0, 0.0, 0.0], width: 0.01, highlighter: false, points }
    }

    #[test]
    fn marks_land_where_they_were_drawn() {
        // A4 portrait: the top left of the shown page is the box's top left.
        let content = marks_content(&[mark(vec![[0.0, 0.0], [1.0, 1.0]])], [0.0, 0.0, 595.0, 842.0], 0);
        assert!(content.contains("1.000 0.000 0.000 RG 5.95 w"));
        assert!(content.contains("0.00 842.00 m\n595.00 0.00 l"));
        // Turned a quarter: the shown top left is the box's bottom left, and widths follow the shown width.
        let content = marks_content(&[mark(vec![[0.0, 0.0], [1.0, 0.0]])], [0.0, 0.0, 595.0, 842.0], 90);
        assert!(content.contains("RG 8.42 w"));
        assert!(content.contains("0.00 0.00 m\n0.00 842.00 l"));
    }

    #[test]
    fn highlighters_are_see_through_and_dots_stay_visible() {
        let mut highlight = mark(vec![[0.5, 0.5]]);
        highlight.highlighter = true;
        let content = marks_content(&[highlight], [0.0, 0.0, 100.0, 100.0], 0);
        assert!(content.contains("/NwHl gs"));
        assert!(content.contains("50.00 50.00 m\n50.01 50.00 l"));
    }
}
