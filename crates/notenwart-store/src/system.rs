// SPDX-License-Identifier: AGPL-3.0-or-later

//! The data of `notenwart_app::system`: its queries, one function each, checked at compile time against
//! `.sqlx/`.

use uuid::Uuid;

#[derive(Debug)]
pub struct OrganizationchoiceRow {
    pub id: Uuid,
    pub name: String,
}

#[derive(Debug)]
pub struct SystemRow {
    pub people: i64,
    pub ensembles: i64,
    pub works: i64,
    pub parts: i64,
    pub recordings: i64,
    pub bytes: i64,
    pub logo: bool,
    pub setup_done: Vec<String>,
}

pub async fn organizations(db: impl sqlx::PgExecutor<'_>) -> sqlx::Result<Vec<OrganizationchoiceRow>> {
    sqlx::query_as!(OrganizationchoiceRow, "SELECT id, name FROM organizations ORDER BY created_at").fetch_all(db).await
}

pub async fn system(db: impl sqlx::PgExecutor<'_>, org: Uuid) -> sqlx::Result<SystemRow> {
    sqlx::query_as!(SystemRow, r#"SELECT (SELECT count(*) FROM memberships WHERE organization_id = $1) AS "people!",
                  (SELECT count(*) FROM ensembles WHERE organization_id = $1) AS "ensembles!",
                  (SELECT count(*) FROM works WHERE organization_id = $1) AS "works!",
                  (SELECT count(*) FROM parts p JOIN works w ON w.id = p.work_id WHERE w.organization_id = $1) AS "parts!",
                  (SELECT count(*) FROM recordings WHERE organization_id = $1) AS "recordings!",
                  (SELECT coalesce(sum(s.size), 0) FROM source_files s JOIN works w ON w.id = s.work_id
                     WHERE w.organization_id = $1)::bigint
                  + (SELECT coalesce(sum(size), 0) FROM recordings WHERE organization_id = $1)::bigint
                  + (SELECT coalesce(sum(size), 0) FROM images WHERE organization_id = $1)::bigint AS "bytes!",
                  o.logo_id IS NOT NULL AS "logo!", o.setup_done
           FROM organizations o WHERE o.id = $1"#, org).fetch_one(db).await
}

pub async fn mark(
    db: impl sqlx::PgExecutor<'_>,
    organization_id: Uuid,
    key: &str,
    done: bool,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!(
        "UPDATE organizations SET setup_done = CASE WHEN $3 THEN array_append(array_remove(setup_done, $2), $2)
                                                  ELSE array_remove(setup_done, $2) END
         WHERE id = $1",
        organization_id,
        key,
        done
    )
    .execute(db)
    .await
}
