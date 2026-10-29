// SPDX-License-Identifier: AGPL-3.0-or-later

//! How some plain strings look in the API description: the values they can take. Used by `#[schema(schema_with)]`
//! where a field is a string in Rust but one of a few values on the wire.

use utoipa::openapi::schema::{ObjectBuilder, Type};

/// The values a string can take.
pub trait Strings {
    const VALUES: &[&str];
}

/// A string that is one of `T::VALUES`.
pub fn strings<T: Strings>() -> utoipa::openapi::Object {
    ObjectBuilder::new().schema_type(Type::String).enum_values(Some(T::VALUES.iter().copied())).build()
}

/// The same, or nothing.
pub fn optional_strings<T: Strings>() -> utoipa::openapi::Object {
    ObjectBuilder::new()
        .schema_type(utoipa::openapi::schema::SchemaType::from_iter([Type::String, Type::Null]))
        .enum_values(Some(T::VALUES.iter().copied()))
        .build()
}
