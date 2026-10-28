// SPDX-License-Identifier: AGPL-3.0-or-later

mod common;

use common::{page_widths, sample_pdf};
use notenwart::pdf::{PdfError, extract, page_count};

#[test]
fn counts_pages() {
    assert_eq!(page_count(&sample_pdf(3)).unwrap(), 3);
}

#[test]
fn rejects_non_pdf() {
    assert!(matches!(page_count(b"hello"), Err(PdfError::Unreadable(_))));
}

#[test]
fn extracts_pages_in_given_order_with_inherited_media_box() {
    let part = extract(&sample_pdf(4), &[3, 1]).unwrap();
    assert_eq!(page_count(&part).unwrap(), 2);
    assert_eq!(page_widths(&part), vec![300, 100]);
}

#[test]
fn missing_page_is_an_error() {
    assert!(matches!(extract(&sample_pdf(2), &[3]), Err(PdfError::NoSuchPage(3))));
}
