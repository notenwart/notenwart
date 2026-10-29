// SPDX-License-Identifier: AGPL-3.0-or-later

//! The data of `notenwart_app::ensembles`: its queries, one function each, checked at compile time against
//! `.sqlx/`.

use notenwart_core::Role;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug)]
pub struct SummariesRow {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub logo_url: Option<String>,
    pub symbol: Option<String>,
    pub color: Option<String>,
    pub member_count: i64,
    pub work_count: i64,
    pub leader: bool,
    pub member: bool,
}

#[derive(Debug)]
pub struct MembersRow {
    pub user_id: Uuid,
    pub display_name: String,
    pub leader: bool,
    pub avatar_url: Option<String>,
    pub instruments: Vec<Uuid>,
    pub voices: Vec<i16>,
}

#[derive(Debug)]
pub struct EnsembleworkRow {
    pub id: Uuid,
    pub title: String,
    pub composer: Option<String>,
    pub arranger: Option<String>,
    pub publisher: Option<String>,
    pub lyricist: Option<String>,
    pub society_number: Option<String>,
    pub duration_seconds: Option<i32>,
}

#[derive(Debug)]
pub struct NotifyWorkGivenRow {
    pub user_id: Uuid,
    pub work_id: Uuid,
    pub title: String,
}

#[derive(Debug)]
pub struct MyMusicRow {
    pub id: Uuid,
    pub name: String,
    pub leader: bool,
    pub logo_url: Option<String>,
    pub symbol: Option<String>,
    pub color: Option<String>,
}

#[derive(Debug)]
pub struct MyMusic2Row {
    pub ensemble_id: Uuid,
    pub work_id: Uuid,
    pub title: String,
    pub composer: Option<String>,
    pub part_id: Uuid,
    pub label: String,
    pub page_count: i32,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug)]
pub struct PartInfoRow {
    pub id: Uuid,
    pub label: String,
    pub page_count: i32,
    pub updated_at: OffsetDateTime,
    pub work_id: Uuid,
    pub title: String,
    pub composer: Option<String>,
}

pub async fn can_manage(db: impl sqlx::PgExecutor<'_>, ensemble_id: Uuid, id: Uuid) -> sqlx::Result<bool> {
    sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM ensemble_members WHERE ensemble_id = $1 AND user_id = $2 AND leader) AS "x!""#,
        ensemble_id,
        id
    )
    .fetch_one(db)
    .await
}

pub async fn check_visible(
    db: impl sqlx::PgExecutor<'_>,
    ensemble_id: Uuid,
    organization_id: Uuid,
    id: Uuid,
    is_staff: bool,
) -> sqlx::Result<bool> {
    sqlx::query_scalar!(
        r#"SELECT EXISTS (
               SELECT 1 FROM ensembles e WHERE e.id = $1 AND e.organization_id = $2
                 AND ($4 OR EXISTS (SELECT 1 FROM ensemble_members m WHERE m.ensemble_id = e.id AND m.user_id = $3))
           ) AS "x!""#,
        ensemble_id,
        organization_id,
        id,
        is_staff
    )
    .fetch_one(db)
    .await
}

pub async fn summaries(
    db: impl sqlx::PgExecutor<'_>,
    organization_id: Uuid,
    id: Uuid,
    only: Option<Uuid>,
) -> sqlx::Result<Vec<SummariesRow>> {
    sqlx::query_as!(
        SummariesRow,
        r#"SELECT e.id, e.name, e.description, '/api/images/' || e.logo_id AS logo_url, e.symbol, e.color,
                  (SELECT count(*) FROM ensemble_members m WHERE m.ensemble_id = e.id) AS "member_count!",
                  (SELECT count(*) FROM ensemble_works w WHERE w.ensemble_id = e.id) AS "work_count!",
                  coalesce((SELECT leader FROM ensemble_members m WHERE m.ensemble_id = e.id AND m.user_id = $2),
                           false) AS "leader!",
                  EXISTS (SELECT 1 FROM ensemble_members m WHERE m.ensemble_id = e.id AND m.user_id = $2)
                      AS "member!"
           FROM ensembles e
           WHERE e.organization_id = $1 AND ($3::uuid IS NULL OR e.id = $3)
           ORDER BY lower(e.name)"#,
        organization_id,
        id,
        only
    )
    .fetch_all(db)
    .await
}

pub async fn create(
    db: impl sqlx::PgExecutor<'_>,
    organization_id: Uuid,
    name: &str,
    description: Option<&str>,
) -> sqlx::Result<Uuid> {
    sqlx::query_scalar!(
        "INSERT INTO ensembles (organization_id, name, description) VALUES ($1, $2, $3) RETURNING id",
        organization_id,
        name,
        description
    )
    .fetch_one(db)
    .await
}

pub async fn update(
    db: impl sqlx::PgExecutor<'_>,
    ensemble_id: Uuid,
    name: &str,
    description: Option<&str>,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!("UPDATE ensembles SET name = $2, description = $3 WHERE id = $1", ensemble_id, name, description)
        .execute(db)
        .await
}

pub async fn remove(
    db: impl sqlx::PgExecutor<'_>,
    ensemble_id: Uuid,
    organization_id: Uuid,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!("DELETE FROM ensembles WHERE id = $1 AND organization_id = $2", ensemble_id, organization_id)
        .execute(db)
        .await
}

pub async fn members(
    db: impl sqlx::PgExecutor<'_>,
    ensemble_id: Uuid,
    only: Option<Uuid>,
) -> sqlx::Result<Vec<MembersRow>> {
    sqlx::query_as!(
        MembersRow,
        r#"SELECT m.user_id, u.display_name, m.leader,
                  CASE WHEN o.profile_pictures THEN '/api/images/' || u.avatar_id END AS avatar_url,
                  coalesce(array_agg(p.instrument_id) FILTER (WHERE p.instrument_id IS NOT NULL), '{}')
                      AS "instruments!: Vec<Uuid>",
                  coalesce(array_agg(coalesce(p.voice, 0)) FILTER (WHERE p.instrument_id IS NOT NULL), '{}')
                      AS "voices!: Vec<i16>"
           FROM ensemble_members m
           JOIN users u ON u.id = m.user_id
           JOIN ensembles e2 ON e2.id = m.ensemble_id JOIN organizations o ON o.id = e2.organization_id
           LEFT JOIN ensemble_member_parts p ON p.ensemble_id = m.ensemble_id AND p.user_id = m.user_id
           WHERE m.ensemble_id = $1 AND ($2::uuid IS NULL OR m.user_id = $2)
           GROUP BY m.user_id, u.display_name, m.leader, o.profile_pictures, u.avatar_id
           ORDER BY m.leader DESC, lower(u.display_name)"#,
        ensemble_id,
        only
    )
    .fetch_all(db)
    .await
}

pub async fn detail(db: impl sqlx::PgExecutor<'_>, ensemble_id: Uuid) -> sqlx::Result<Vec<EnsembleworkRow>> {
    sqlx::query_as!(
        EnsembleworkRow,
        "SELECT w.id, w.title, w.composer, w.arranger, w.publisher, w.lyricist, w.society_number, w.duration_seconds
         FROM ensemble_works ew JOIN works w ON w.id = ew.work_id
         WHERE ew.ensemble_id = $1 ORDER BY lower(w.title)",
        ensemble_id
    )
    .fetch_all(db)
    .await
}

pub async fn put_member_select_memberships(
    db: impl sqlx::PgExecutor<'_>,
    member_id: Uuid,
    organization_id: Uuid,
) -> sqlx::Result<Option<Role>> {
    sqlx::query_scalar!(
        r#"SELECT role AS "role: Role" FROM memberships WHERE user_id = $1 AND organization_id = $2"#,
        member_id,
        organization_id
    )
    .fetch_optional(db)
    .await
}

pub async fn put_member_select_ensemble_members(
    db: impl sqlx::PgExecutor<'_>,
    ensemble_id: Uuid,
    member_id: Uuid,
) -> sqlx::Result<Option<bool>> {
    sqlx::query_scalar!(
        "SELECT leader FROM ensemble_members WHERE ensemble_id = $1 AND user_id = $2",
        ensemble_id,
        member_id
    )
    .fetch_optional(db)
    .await
}

pub async fn put_member_select_instruments(
    db: impl sqlx::PgExecutor<'_>,
    instruments: &[Uuid],
    organization_id: Uuid,
) -> sqlx::Result<i64> {
    sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM instruments
           WHERE id = ANY($1) AND organization_id = $2"#,
        instruments,
        organization_id
    )
    .fetch_one(db)
    .await
}

pub async fn put_member_insert_ensemble_members(
    db: impl sqlx::PgExecutor<'_>,
    ensemble_id: Uuid,
    member_id: Uuid,
    leader: bool,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!(
        "INSERT INTO ensemble_members (ensemble_id, user_id, leader) VALUES ($1, $2, $3)
         ON CONFLICT (ensemble_id, user_id) DO UPDATE SET leader = excluded.leader",
        ensemble_id,
        member_id,
        leader
    )
    .execute(db)
    .await
}

pub async fn put_member_delete_ensemble_member_parts(
    db: impl sqlx::PgExecutor<'_>,
    ensemble_id: Uuid,
    member_id: Uuid,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!("DELETE FROM ensemble_member_parts WHERE ensemble_id = $1 AND user_id = $2", ensemble_id, member_id)
        .execute(db)
        .await
}

pub async fn put_member_insert_ensemble_member_parts(
    db: impl sqlx::PgExecutor<'_>,
    ensemble_id: Uuid,
    member_id: Uuid,
    instruments: &[Uuid],
    voices: &[Option<i16>],
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!(
        "INSERT INTO ensemble_member_parts (ensemble_id, user_id, instrument_id, voice)
         SELECT $1, $2, i, v FROM unnest($3::uuid[], $4::smallint[]) AS t(i, v)",
        ensemble_id,
        member_id,
        instruments,
        voices as &[Option<i16>]
    )
    .execute(db)
    .await
}

pub async fn remove_member_select_ensemble_members(
    db: impl sqlx::PgExecutor<'_>,
    ensemble_id: Uuid,
    member_id: Uuid,
) -> sqlx::Result<Option<bool>> {
    sqlx::query_scalar!(
        "SELECT leader FROM ensemble_members WHERE ensemble_id = $1 AND user_id = $2",
        ensemble_id,
        member_id
    )
    .fetch_optional(db)
    .await
}

pub async fn remove_member_delete_ensemble_members(
    db: impl sqlx::PgExecutor<'_>,
    ensemble_id: Uuid,
    member_id: Uuid,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!("DELETE FROM ensemble_members WHERE ensemble_id = $1 AND user_id = $2", ensemble_id, member_id)
        .execute(db)
        .await
}

pub async fn notify_work_given(
    db: impl sqlx::PgExecutor<'_>,
    ensemble_id: Uuid,
    works: &[Uuid],
    by: Uuid,
) -> sqlx::Result<Vec<NotifyWorkGivenRow>> {
    sqlx::query_as!(
        NotifyWorkGivenRow,
        r#"SELECT DISTINCT v.user_id AS "user_id!", w.id AS work_id, w.title
               FROM visible_parts v JOIN parts p ON p.id = v.part_id JOIN works w ON w.id = p.work_id
               WHERE v.ensemble_id = $1 AND p.work_id = ANY($2) AND v.user_id <> $3"#,
        ensemble_id,
        works,
        by
    )
    .fetch_all(db)
    .await
}

pub async fn add_work_select_works(
    db: impl sqlx::PgExecutor<'_>,
    work_id: Uuid,
    organization_id: Uuid,
) -> sqlx::Result<bool> {
    sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM works WHERE id = $1 AND organization_id = $2) AS "x!""#,
        work_id,
        organization_id
    )
    .fetch_one(db)
    .await
}

pub async fn add_work_insert_ensemble_works(
    db: impl sqlx::PgExecutor<'_>,
    ensemble_id: Uuid,
    work_id: Uuid,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!(
        "INSERT INTO ensemble_works (ensemble_id, work_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
        ensemble_id,
        work_id
    )
    .execute(db)
    .await
}

pub async fn remove_work_delete_ensemble_works(
    db: impl sqlx::PgExecutor<'_>,
    ensemble_id: Uuid,
    work_id: Uuid,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!("DELETE FROM ensemble_works WHERE ensemble_id = $1 AND work_id = $2", ensemble_id, work_id)
        .execute(db)
        .await
}

pub async fn remove_work_delete_program_works(
    db: impl sqlx::PgExecutor<'_>,
    ensemble_id: Uuid,
    work_id: Uuid,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!(
        "DELETE FROM program_works pw USING programs p
         WHERE pw.program_id = p.id AND p.ensemble_id = $1 AND pw.work_id = $2",
        ensemble_id,
        work_id
    )
    .execute(db)
    .await
}

pub async fn remove_work_delete_part_grants(
    db: impl sqlx::PgExecutor<'_>,
    ensemble_id: Uuid,
    work_id: Uuid,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!(
        "DELETE FROM part_grants g USING parts p WHERE g.part_id = p.id AND g.ensemble_id = $1 AND p.work_id = $2",
        ensemble_id,
        work_id
    )
    .execute(db)
    .await
}

pub async fn my_music_select_ensemble_members(
    db: impl sqlx::PgExecutor<'_>,
    id: Uuid,
    organization_id: Uuid,
) -> sqlx::Result<Vec<MyMusicRow>> {
    sqlx::query_as!(
        MyMusicRow,
        "SELECT e.id, e.name, m.leader, '/api/images/' || e.logo_id AS logo_url, e.symbol, e.color
         FROM ensemble_members m JOIN ensembles e ON e.id = m.ensemble_id
         WHERE m.user_id = $1 AND e.organization_id = $2 ORDER BY lower(e.name)",
        id,
        organization_id
    )
    .fetch_all(db)
    .await
}

pub async fn my_music_select_visible_parts(db: impl sqlx::PgExecutor<'_>, id: Uuid) -> sqlx::Result<Vec<MyMusic2Row>> {
    sqlx::query_as!(
        MyMusic2Row,
        r#"SELECT v.ensemble_id AS "ensemble_id!", w.id AS work_id, w.title, w.composer, p.id AS part_id, p.label,
                  cardinality(p.pages) AS "page_count!", p.updated_at
           FROM visible_parts v
           JOIN parts p ON p.id = v.part_id
           JOIN works w ON w.id = p.work_id
           LEFT JOIN instruments i ON i.id = p.instrument_id
           WHERE v.user_id = $1
           ORDER BY lower(w.title), w.id, i.sort NULLS FIRST, p.voice NULLS FIRST, lower(p.label)"#,
        id
    )
    .fetch_all(db)
    .await
}

pub async fn may_see_part(db: impl sqlx::PgExecutor<'_>, id: Uuid, part_id: Uuid) -> sqlx::Result<bool> {
    sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM visible_parts WHERE user_id = $1 AND part_id = $2) AS "x!""#,
        id,
        part_id
    )
    .fetch_one(db)
    .await
}

pub async fn part_info(
    db: impl sqlx::PgExecutor<'_>,
    part_id: Uuid,
    organization_id: Uuid,
) -> sqlx::Result<Option<PartInfoRow>> {
    sqlx::query_as!(
        PartInfoRow,
        r#"SELECT p.id, p.label, cardinality(p.pages) AS "page_count!", p.updated_at, w.id AS work_id,
                  w.title, w.composer
           FROM parts p JOIN works w ON w.id = p.work_id
           WHERE p.id = $1 AND w.organization_id = $2"#,
        part_id,
        organization_id
    )
    .fetch_optional(db)
    .await
}
