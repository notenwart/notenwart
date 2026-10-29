// SPDX-License-Identifier: AGPL-3.0-or-later

//! The description of the API in OpenAPI, from the code: every handler carries its `#[utoipa::path]`, every type it
//! takes or gives derives `ToSchema`. `notenwart openapi` prints it; `api/openapi.json` holds it (MIT, so clients
//! under any licence can be generated from it), and CI checks that the two match.

use utoipa::openapi::security::{ApiKey, ApiKeyValue, SecurityScheme};
use utoipa::{Modify, OpenApi, ToResponse, ToSchema};

use crate::error::Problem;

/// The description of the API.
#[derive(Debug, OpenApi)]
#[openapi(
    info(
        title = "notenwart API",
        description = "Sign-in sets an HTTP-only session cookie; every endpoint except `/health` and `/auth/*` needs it \
and acts on\nthe signed-in user's organisation. Requests that change something and carry an `Origin` header must come \
from\nthe server's own origin.\n\nRoles: **admin** (everything), **librarian** (library and ensembles), **leader** \
(reads the library, manages\nthe ensembles they lead), **member** (sees their own parts). A member sees a part when one \
of their ensembles\nhas the part's work and they play its instrument there – any part number, or the same one – or \
when a leader\ngranted them that part in the ensemble; parts without an instrument (full score, …) are for leaders.\n",
        license(name = "MIT", identifier = "MIT")
    ),
    servers((url = "/api")),
    security(("session" = [])),
    tags(
        (name = "auth", description = "Setup, sign-in and the signed-in user."),
        (name = "people", description = "The organisation's users (admin)."),
        (name = "ensembles", description = "Ensembles, their members with instruments, and the works they play."),
        (name = "library", description = "Works, their uploaded source PDFs and the parts split from them."),
        (name = "system", description = "For admins – what the server is and can do, a test email, and the setup checklist."),
        (name = "images", description = "The organisation's logo, ensemble logos and symbols, people's pictures."),
        (name = "recordings", description = "Full and practice recordings of works (audio only)."),
        (name = "guests", description = "Links for substitutes without an account to chosen parts of an ensemble, until a date."),
        (name = "notifications", description = "What happened for the signed-in user, and how they want to hear about it."),
        (name = "privacy", description = "What is stored about a person, and the record of deleted people."),
        (name = "loans", description = "The paper archive's loans – who has borrowed which printed parts."),
        (name = "catalogue", description = "The organisation's instrument families and instruments. A new organisation starts \
with a copy of the built-in catalogue, named in the server's language.")
    ),
    paths(
        crate::health,
        crate::auth::status,
        crate::auth::setup,
        crate::auth::login,
        crate::auth::magic_link,
        crate::auth::redeem,
        crate::auth::logout,
        crate::auth::me,
        crate::auth::update_me,
        crate::programs::create,
        crate::programs::update,
        crate::programs::remove,
        crate::programs::set_items,
        crate::programs::ensemble_parts,
        crate::programs::grants,
        crate::programs::grant,
        crate::programs::revoke,
        crate::ensembles::my_music,
        crate::people::list,
        crate::people::create,
        crate::import::import,
        crate::people::bulk,
        crate::people::update,
        crate::people::remove,
        crate::people::invite,
        crate::ensembles::list,
        crate::ensembles::create,
        crate::ensembles::detail,
        crate::ensembles::update,
        crate::ensembles::remove,
        crate::ensembles::put_member,
        crate::ensembles::remove_member,
        crate::ensembles::add_work,
        crate::ensembles::remove_work,
        crate::catalogue::list_instruments,
        crate::catalogue::create_instrument,
        crate::catalogue::update_instrument,
        crate::catalogue::delete_instrument,
        crate::catalogue::merge_instrument,
        crate::catalogue::list_families,
        crate::catalogue::create_family,
        crate::catalogue::update_family,
        crate::catalogue::delete_family,
        crate::library::list_tags,
        crate::library::recognize,
        crate::loans::list,
        crate::loans::update,
        crate::loans::remove,
        crate::loans::for_work,
        crate::loans::create,
        crate::loans::mine,
        crate::loans::borrowers,
        crate::sso::start,
        crate::sso::callback,
        crate::system::system,
        crate::system::test_mail,
        crate::system::tick,
        crate::images::image,
        crate::images::organization,
        crate::images::update_organization,
        crate::images::organization_logo,
        crate::images::remove_organization_logo,
        crate::images::ensemble_logo,
        crate::images::remove_ensemble_logo,
        crate::images::ensemble_symbol,
        crate::images::my_avatar,
        crate::images::remove_my_avatar,
        crate::recordings::list,
        crate::recordings::upload,
        crate::recordings::update,
        crate::recordings::remove,
        crate::recordings::file,
        crate::guests::list,
        crate::guests::create,
        crate::guests::revoke,
        crate::guests::open,
        crate::guests::file,
        crate::notify::inbox,
        crate::notify::mark_read,
        crate::notify::settings,
        crate::notify::save_settings,
        crate::privacy::export_mine,
        crate::privacy::export_user,
        crate::privacy::deletions,
        crate::library::work_facets,
        crate::library::list_works,
        crate::library::create_work,
        crate::library::get_work,
        crate::library::update_work,
        crate::library::delete_work,
        crate::library::upload_source,
        crate::library::delete_source,
        crate::library::recognize_source,
        crate::library::split_source,
        crate::library::source_file,
        crate::library::create_part,
        crate::library::update_part,
        crate::library::delete_part,
        crate::ensembles::part_info,
        crate::annotations::load_shared,
        crate::annotations::save_shared,
        crate::annotations::annotated,
        crate::annotations::load,
        crate::annotations::save,
        crate::library::part_file
    ),
    components(responses(Unauthorized, Forbidden, Conflict, TooManyRequests, NotFound, Invalid)),
    modifiers(&Session)
)]
pub struct ApiDoc;

/// The description as pretty JSON – what `api/openapi.json` holds.
pub fn openapi() -> String {
    ApiDoc::openapi().to_pretty_json().expect("the description in JSON")
}

/// The session cookie that sign-in sets.
#[derive(Debug)]
struct Session;

impl Modify for Session {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi.components.get_or_insert_with(Default::default);
        components.add_security_scheme(
            "session",
            SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::new(crate::auth::SESSION_COOKIE))),
        );
    }
}

// The types below only describe requests and answers; nothing builds them.

/// Not signed in, or the credentials or token are wrong or expired.
#[derive(Debug, ToResponse)]
#[allow(dead_code)]
pub struct Unauthorized(Problem);

/// Signed in, but the role does not allow this.
#[derive(Debug, ToResponse)]
#[allow(dead_code)]
pub struct Forbidden(Problem);

/// Clashes with existing data, e.g. a name or email that is taken.
#[derive(Debug, ToResponse)]
#[allow(dead_code)]
pub struct Conflict(Problem);

/// Too many attempts; try again later.
#[derive(Debug, ToResponse)]
#[allow(dead_code)]
pub struct TooManyRequests(Problem);

/// No such object in this organisation.
#[derive(Debug, ToResponse)]
#[allow(dead_code)]
pub struct NotFound(Problem);

/// The request is well-formed but its content is not acceptable.
#[derive(Debug, ToResponse)]
#[allow(dead_code)]
pub struct Invalid(Problem);

/// A file as it is sent or received: any bytes.
#[derive(Debug, ToSchema)]
#[schema(value_type = String, format = Binary)]
#[allow(dead_code)]
pub struct Binary(Vec<u8>);

/// A form with one file. It only describes the request; the handler reads the form itself.
#[derive(Debug, ToSchema)]
pub struct FileForm {
    /// One file. The server's upload limit applies (413 when exceeded).
    #[schema(value_type = String, format = Binary)]
    pub file: Vec<u8>,
}

/// The form of a new recording. It only describes the request; the handler reads the form itself.
#[derive(Debug, ToSchema)]
pub struct RecordingForm {
    /// The audio file.
    #[schema(value_type = String, format = Binary)]
    pub file: Vec<u8>,
    /// RecordingInput as JSON.
    pub meta: String,
}

/// A self-describing export (account, ensembles, singleParts, annotations, loans, signIns); its fields may grow.
/// It only describes the answer; the app builds it as JSON.
#[derive(Debug, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PersonalData {
    #[schema(value_type = String, format = DateTime)]
    pub exported_at: String,
    #[schema(value_type = Object)]
    pub account: serde_json::Value,
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_description_names_every_route() {
        let description = super::openapi();
        for path in ["/health", "/auth/login", "/works/{workId}", "/parts/{partId}/file"] {
            assert!(description.contains(path), "{path}");
        }
    }
}
