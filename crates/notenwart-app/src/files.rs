// SPDX-License-Identifier: AGPL-3.0-or-later

//! Files as an interface hands them in: the API reads them from multipart forms.

use bytes::Bytes;

/// An uploaded file: the name it had, if any, and its content.
pub struct Upload {
    pub file_name: Option<String>,
    pub bytes: Bytes,
}

impl std::fmt::Debug for Upload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Upload").field("file_name", &self.file_name).field("size", &self.bytes.len()).finish()
    }
}
