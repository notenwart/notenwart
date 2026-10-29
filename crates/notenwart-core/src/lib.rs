// SPDX-License-Identifier: AGPL-3.0-or-later

//! The domain of notenwart: its types and rules, without I/O – the roles of people, the texts of emails, and what
//! is done to PDFs (counting and extracting pages, drawing annotations). `notenwart-app` builds on it; with the
//! feature `sqlx`, its types are also stored as they are.

pub mod email;
pub mod pdf;

use serde::{Deserialize, Serialize};

/// What someone may do in an organisation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
#[cfg_attr(feature = "sqlx", sqlx(type_name = "organization_role", rename_all = "lowercase"))]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Admin,
    Librarian,
    Leader,
    Member,
}
