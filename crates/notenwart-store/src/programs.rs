// SPDX-License-Identifier: AGPL-3.0-or-later

//! The data of `notenwart_app::programs`: its queries, one function each, checked at compile time against
//! `.sqlx/`.

use uuid::Uuid;

#[derive(Debug)]
pub struct ProgramsForRow {
    pub id: Uuid,
    pub ensemble_id: Uuid,
    pub name: String,
    pub date: Option<String>,
    pub start_time: Option<String>,
    pub venue: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug)]
pub struct ProgramsFor2Row {
    pub program_id: Uuid,
    pub work_id: Uuid,
    pub position: i32,
    pub note: Option<String>,
}

#[derive(Debug)]
pub struct ProgramsFor3Row {
    pub program_id: Uuid,
    pub position: i32,
    pub label: String,
    pub duration_seconds: Option<i32>,
    pub note: Option<String>,
}

#[derive(Debug)]
pub struct EnsemblepartRow {
    pub id: Uuid,
    pub label: String,
    pub instrument_id: Option<Uuid>,
    pub voice: Option<i16>,
    pub work_id: Uuid,
    pub work_title: String,
}

#[derive(Debug)]
pub struct GrantRow {
    pub part_id: Uuid,
    pub user_id: Uuid,
    pub label: String,
    pub work_title: String,
    pub user_name: String,
}

pub async fn programs_for_select_programs(
    db: impl sqlx::PgExecutor<'_>,
    ensembles: &[Uuid],
) -> sqlx::Result<Vec<ProgramsForRow>> {
    sqlx::query_as!(ProgramsForRow, r#"SELECT id, ensemble_id, name, to_char(date, 'YYYY-MM-DD') AS date, to_char(start_time, 'HH24:MI') AS start_time,
                  venue, notes
           FROM programs WHERE ensemble_id = ANY($1)
           ORDER BY date NULLS LAST, lower(name)"#, ensembles).fetch_all(db).await
}

pub async fn programs_for_select_program_works(
    db: impl sqlx::PgExecutor<'_>,
    ids: &[Uuid],
) -> sqlx::Result<Vec<ProgramsFor2Row>> {
    sqlx::query_as!(
        ProgramsFor2Row,
        "SELECT program_id, work_id, position, note FROM program_works WHERE program_id = ANY($1)",
        ids
    )
    .fetch_all(db)
    .await
}

pub async fn programs_for_select_program_breaks(
    db: impl sqlx::PgExecutor<'_>,
    ids: &[Uuid],
) -> sqlx::Result<Vec<ProgramsFor3Row>> {
    sqlx::query_as!(
        ProgramsFor3Row,
        "SELECT program_id, position, label, duration_seconds, note FROM program_breaks WHERE program_id = ANY($1)",
        ids
    )
    .fetch_all(db)
    .await
}

pub async fn find(
    db: impl sqlx::PgExecutor<'_>,
    program_id: Uuid,
    organization_id: Uuid,
) -> sqlx::Result<Option<Uuid>> {
    sqlx::query_scalar!(
        "SELECT p.ensemble_id FROM programs p JOIN ensembles e ON e.id = p.ensemble_id
         WHERE p.id = $1 AND e.organization_id = $2",
        program_id,
        organization_id
    )
    .fetch_optional(db)
    .await
}

pub async fn notify_program(db: impl sqlx::PgExecutor<'_>, ensemble_id: Uuid) -> sqlx::Result<Option<String>> {
    sqlx::query_scalar!("SELECT name FROM ensembles WHERE id = $1", ensemble_id).fetch_optional(db).await
}

pub async fn create(
    db: impl sqlx::PgExecutor<'_>,
    ensemble_id: Uuid,
    name: &str,
    date: Option<&str>,
    start_time: Option<&str>,
    venue: Option<&str>,
    notes: Option<&str>,
) -> sqlx::Result<Uuid> {
    sqlx::query_scalar!(
        "INSERT INTO programs (ensemble_id, name, date, start_time, venue, notes)
         VALUES ($1, $2, $3::text::date, $4::text::time, $5, $6) RETURNING id",
        ensemble_id,
        name,
        date,
        start_time,
        venue,
        notes
    )
    .fetch_one(db)
    .await
}

pub async fn update(
    db: impl sqlx::PgExecutor<'_>,
    program_id: Uuid,
    name: &str,
    date: Option<&str>,
    start_time: Option<&str>,
    venue: Option<&str>,
    notes: Option<&str>,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!(
        "UPDATE programs SET name = $2, date = $3::text::date, start_time = $4::text::time, venue = $5, notes = $6
         WHERE id = $1",
        program_id,
        name,
        date,
        start_time,
        venue,
        notes
    )
    .execute(db)
    .await
}

pub async fn remove(db: impl sqlx::PgExecutor<'_>, program_id: Uuid) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!("DELETE FROM programs WHERE id = $1", program_id).execute(db).await
}

pub async fn set_items_select_works(
    db: impl sqlx::PgExecutor<'_>,
    works: &[Uuid],
    organization_id: Uuid,
) -> sqlx::Result<i64> {
    sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM works WHERE id = ANY($1) AND organization_id = $2"#,
        works,
        organization_id
    )
    .fetch_one(db)
    .await
}

pub async fn set_items_insert_ensemble_works(
    db: impl sqlx::PgExecutor<'_>,
    ensemble_id: Uuid,
    works: &[Uuid],
) -> sqlx::Result<Vec<Uuid>> {
    sqlx::query_scalar!(
        "INSERT INTO ensemble_works (ensemble_id, work_id) SELECT $1, w FROM unnest($2::uuid[]) AS w
         ON CONFLICT DO NOTHING RETURNING work_id",
        ensemble_id,
        works
    )
    .fetch_all(db)
    .await
}

pub async fn set_items_delete_program_works(
    db: impl sqlx::PgExecutor<'_>,
    program_id: Uuid,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!("DELETE FROM program_works WHERE program_id = $1", program_id).execute(db).await
}

pub async fn set_items_delete_program_breaks(
    db: impl sqlx::PgExecutor<'_>,
    program_id: Uuid,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!("DELETE FROM program_breaks WHERE program_id = $1", program_id).execute(db).await
}

pub async fn set_items_insert_program_works(
    db: impl sqlx::PgExecutor<'_>,
    program_id: Uuid,
    works: &[Uuid],
    work_positions: &[i32],
    work_notes: &[Option<String>],
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!(
        "INSERT INTO program_works (program_id, work_id, position, note)
         SELECT $1, w, p, n FROM unnest($2::uuid[], $3::int[], $4::text[]) AS t(w, p, n)",
        program_id,
        works,
        work_positions,
        work_notes as &[Option<String>]
    )
    .execute(db)
    .await
}

pub async fn set_items_insert_program_breaks(
    db: impl sqlx::PgExecutor<'_>,
    program_id: Uuid,
    break_positions: &[i32],
    labels: &[String],
    durations: &[Option<i32>],
    break_notes: &[Option<String>],
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!(
        "INSERT INTO program_breaks (program_id, position, label, duration_seconds, note)
         SELECT $1, p, l, d, n FROM unnest($2::int[], $3::text[], $4::int[], $5::text[]) AS t(p, l, d, n)",
        program_id,
        break_positions,
        labels,
        durations as &[Option<i32>],
        break_notes as &[Option<String>]
    )
    .execute(db)
    .await
}

pub async fn ensemble_parts(db: impl sqlx::PgExecutor<'_>, ensemble_id: Uuid) -> sqlx::Result<Vec<EnsemblepartRow>> {
    sqlx::query_as!(
        EnsemblepartRow,
        r#"SELECT p.id, p.label, p.instrument_id, p.voice, w.id AS work_id, w.title AS work_title
           FROM ensemble_works ew JOIN works w ON w.id = ew.work_id JOIN parts p ON p.work_id = w.id
           LEFT JOIN instruments i ON i.id = p.instrument_id
           WHERE ew.ensemble_id = $1
           ORDER BY lower(w.title), w.id, i.sort NULLS FIRST, p.voice NULLS FIRST, lower(p.label)"#,
        ensemble_id
    )
    .fetch_all(db)
    .await
}

pub async fn grants(db: impl sqlx::PgExecutor<'_>, ensemble_id: Uuid) -> sqlx::Result<Vec<GrantRow>> {
    sqlx::query_as!(
        GrantRow,
        r#"SELECT g.part_id, g.user_id, p.label, w.title AS work_title, u.display_name AS user_name
           FROM part_grants g JOIN parts p ON p.id = g.part_id JOIN works w ON w.id = p.work_id
           JOIN users u ON u.id = g.user_id
           WHERE g.ensemble_id = $1
           ORDER BY lower(u.display_name), lower(w.title), lower(p.label)"#,
        ensemble_id
    )
    .fetch_all(db)
    .await
}

pub async fn grant_select_parts(db: impl sqlx::PgExecutor<'_>, part_id: Uuid, ensemble_id: Uuid) -> sqlx::Result<bool> {
    sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM parts p JOIN ensemble_works ew ON ew.work_id = p.work_id
                          WHERE p.id = $1 AND ew.ensemble_id = $2) AS "x!""#,
        part_id,
        ensemble_id
    )
    .fetch_one(db)
    .await
}

pub async fn grant_select_ensemble_members(
    db: impl sqlx::PgExecutor<'_>,
    ensemble_id: Uuid,
    member_id: Uuid,
) -> sqlx::Result<bool> {
    sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM ensemble_members WHERE ensemble_id = $1 AND user_id = $2) AS "x!""#,
        ensemble_id,
        member_id
    )
    .fetch_one(db)
    .await
}

pub async fn grant_insert_part_grants(
    db: impl sqlx::PgExecutor<'_>,
    ensemble_id: Uuid,
    member_id: Uuid,
    part_id: Uuid,
    actor: Option<Uuid>,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!(
        "INSERT INTO part_grants (ensemble_id, user_id, part_id, granted_by) VALUES ($1, $2, $3, $4)
         ON CONFLICT DO NOTHING",
        ensemble_id,
        member_id,
        part_id,
        actor
    )
    .execute(db)
    .await
}

pub async fn revoke(
    db: impl sqlx::PgExecutor<'_>,
    ensemble_id: Uuid,
    part_id: Uuid,
    member_id: Uuid,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!(
        "DELETE FROM part_grants WHERE ensemble_id = $1 AND part_id = $2 AND user_id = $3",
        ensemble_id,
        part_id,
        member_id
    )
    .execute(db)
    .await
}
