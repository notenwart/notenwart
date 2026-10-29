// SPDX-License-Identifier: AGPL-3.0-or-later

//! Each organisation's instrument families and instruments (`catalogue` in api/openapi.yaml). An organisation starts
//! with a copy of the built-in catalogue, named in the server's language; admins and librarians change it from there.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::App;
use crate::auth::CurrentUser;
use crate::error::{AppError, AppResult, unique_violation};
use crate::mail::Language;

// ---- the copy of the built-in catalogue ----

/// Built-in families: template key, English and German name.
const FAMILIES: [(&str, &str, &str); 8] = [
    ("woodwind", "Woodwinds", "Holzbläser"),
    ("brass", "Brass", "Blechbläser"),
    ("percussion", "Percussion", "Schlagwerk"),
    ("keyboard", "Keyboards", "Tasteninstrumente"),
    ("strings", "Strings", "Streicher"),
    ("plucked", "Plucked strings", "Zupfinstrumente"),
    ("voice", "Voices", "Gesang"),
    ("other", "Other", "Sonstige"),
];

/// German names of the built-in instruments (migrations/0002_library.sql).
const GERMAN: [(&str, &str); 39] = [
    ("Piccolo", "Piccoloflöte"),
    ("Flute", "Flöte"),
    ("Recorder", "Blockflöte"),
    ("Oboe", "Oboe"),
    ("English horn", "Englischhorn"),
    ("Clarinet", "Klarinette"),
    ("Eb clarinet", "Es-Klarinette"),
    ("Bass clarinet", "Bassklarinette"),
    ("Bassoon", "Fagott"),
    ("Soprano saxophone", "Sopransaxophon"),
    ("Alto saxophone", "Altsaxophon"),
    ("Tenor saxophone", "Tenorsaxophon"),
    ("Baritone saxophone", "Baritonsaxophon"),
    ("Horn", "Horn"),
    ("Trumpet", "Trompete"),
    ("Flugelhorn", "Flügelhorn"),
    ("Cornet", "Kornett"),
    ("Trombone", "Posaune"),
    ("Bass trombone", "Bassposaune"),
    ("Euphonium", "Euphonium"),
    ("Tuba", "Tuba"),
    ("Timpani", "Pauken"),
    ("Percussion", "Schlagwerk"),
    ("Mallets", "Stabspiele"),
    ("Piano", "Klavier"),
    ("Organ", "Orgel"),
    ("Keyboard", "Keyboard"),
    ("Violin", "Violine"),
    ("Viola", "Bratsche"),
    ("Cello", "Violoncello"),
    ("Double bass", "Kontrabass"),
    ("Guitar", "Gitarre"),
    ("Electric bass", "E-Bass"),
    ("Harp", "Harfe"),
    ("Ukulele", "Ukulele"),
    ("Soprano", "Sopran"),
    ("Alto", "Alt"),
    ("Tenor", "Tenor"),
    ("Bass voice", "Bass (Gesang)"),
];

/// Gives an organisation without a catalogue its copy of the built-in one and moves parts and members that still
/// point at built-in instruments over to the copies. Does nothing once the organisation has families.
pub async fn ensure(pool: &sqlx::PgPool, organization_id: Uuid, language: Language) -> sqlx::Result<()> {
    let mut tx = pool.begin().await?;
    // One server start at a time.
    sqlx::query!("SELECT id FROM organizations WHERE id = $1 FOR UPDATE", organization_id).fetch_one(&mut *tx).await?;
    let has = sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM instrument_families WHERE organization_id = $1) AS "has!""#,
        organization_id
    )
    .fetch_one(&mut *tx)
    .await?;
    if has {
        return Ok(());
    }

    let german: HashMap<&str, &str> = GERMAN.into_iter().collect();
    let mut families = HashMap::new();
    for (sort, (key, english, deutsch)) in FAMILIES.into_iter().enumerate() {
        let name = match language {
            Language::En => english,
            Language::De => deutsch,
        };
        let id = sqlx::query_scalar!(
            "INSERT INTO instrument_families (organization_id, name, sort) VALUES ($1, $2, $3) RETURNING id",
            organization_id,
            name,
            (sort as i32 + 1) * 10
        )
        .fetch_one(&mut *tx)
        .await?;
        families.insert(key, id);
    }

    let template = sqlx::query!(
        r#"SELECT id, name, family::text AS "family!", transposition, aliases, sort
           FROM instruments WHERE organization_id IS NULL ORDER BY sort"#
    )
    .fetch_all(&mut *tx)
    .await?;
    for built_in in template {
        let (name, mut aliases) = match (language, german.get(built_in.name.as_str())) {
            // The English name stays findable (file names, imports) as an alias.
            (Language::De, Some(&deutsch)) if deutsch != built_in.name => {
                (deutsch.to_string(), vec![built_in.name.clone()])
            }
            _ => (built_in.name.clone(), vec![]),
        };
        for alias in built_in.aliases {
            if !alias.eq_ignore_ascii_case(&name) && !aliases.iter().any(|a| a.eq_ignore_ascii_case(&alias)) {
                aliases.push(alias);
            }
        }
        let copy = sqlx::query_scalar!(
            "INSERT INTO instruments (organization_id, name, family_id, transposition, aliases, sort)
             VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
            organization_id,
            name,
            families[built_in.family.as_str()],
            transposition(language, built_in.transposition.as_deref()),
            &aliases,
            built_in.sort
        )
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query!(
            "UPDATE parts SET instrument_id = $1
             WHERE instrument_id = $2 AND work_id IN (SELECT id FROM works WHERE organization_id = $3)",
            copy,
            built_in.id,
            organization_id
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query!(
            "UPDATE ensemble_member_parts SET instrument_id = $1
             WHERE instrument_id = $2 AND ensemble_id IN (SELECT id FROM ensembles WHERE organization_id = $3)",
            copy,
            built_in.id,
            organization_id
        )
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await
}

/// German writes B and Es for B flat and E flat.
fn transposition(language: Language, key: Option<&str>) -> Option<String> {
    let key = key?;
    Some(match (language, key) {
        (Language::De, "Bb") => "B".into(),
        (Language::De, "Eb") => "Es".into(),
        _ => key.into(),
    })
}

// ---- families ----

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Family {
    id: Uuid,
    name: String,
    sort: i32,
    instrument_count: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FamilyInput {
    name: String,
    sort: Option<i32>,
}

fn clean_name(name: &str) -> AppResult<String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 80 {
        return Err(AppError::Invalid("a name needs 1 to 80 characters".into()));
    }
    Ok(name.to_string())
}

pub async fn list_families(state: &App, user: &CurrentUser) -> AppResult<Vec<Family>> {
    let families = sqlx::query_as!(
        Family,
        r#"SELECT f.id, f.name, f.sort,
                  (SELECT count(*) FROM instruments i WHERE i.family_id = f.id) AS "instrument_count!"
           FROM instrument_families f WHERE f.organization_id = $1
           ORDER BY f.sort, lower(f.name)"#,
        user.organization_id
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(families)
}

pub async fn create_family(state: &App, user: &CurrentUser, input: FamilyInput) -> AppResult<Family> {
    user.require(user.is_staff())?;
    let name = clean_name(&input.name)?;
    let family = sqlx::query_as!(
        Family,
        r#"INSERT INTO instrument_families (organization_id, name, sort)
           VALUES ($1, $2, coalesce($3, (SELECT coalesce(max(sort), 0) + 10 FROM instrument_families
                                         WHERE organization_id = $1)))
           RETURNING id, name, sort, 0::bigint AS "instrument_count!""#,
        user.organization_id,
        name,
        input.sort
    )
    .fetch_one(&state.pool)
    .await
    .map_err(|e| unique_violation(e, "a family with this name exists"))?;
    Ok(family)
}

pub async fn update_family(state: &App, user: &CurrentUser, family_id: Uuid, input: FamilyInput) -> AppResult<Family> {
    user.require(user.is_staff())?;
    let name = clean_name(&input.name)?;
    let family = sqlx::query_as!(
        Family,
        r#"UPDATE instrument_families f SET name = $3, sort = coalesce($4, f.sort)
           WHERE f.id = $1 AND f.organization_id = $2
           RETURNING f.id, f.name, f.sort,
                     (SELECT count(*) FROM instruments i WHERE i.family_id = f.id) AS "instrument_count!""#,
        family_id,
        user.organization_id,
        name,
        input.sort
    )
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| unique_violation(e, "a family with this name exists"))?
    .ok_or(AppError::NotFound)?;
    Ok(family)
}

pub async fn delete_family(state: &App, user: &CurrentUser, family_id: Uuid) -> AppResult<()> {
    user.require(user.is_staff())?;
    let used =
        sqlx::query_scalar!(r#"SELECT EXISTS (SELECT 1 FROM instruments WHERE family_id = $1) AS "used!""#, family_id)
            .fetch_one(&state.pool)
            .await?;
    if used {
        return Err(AppError::Conflict("the family still has instruments".into()));
    }
    let deleted = sqlx::query!(
        "DELETE FROM instrument_families WHERE id = $1 AND organization_id = $2",
        family_id,
        user.organization_id
    )
    .execute(&state.pool)
    .await?;
    if deleted.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(())
}

// ---- instruments ----

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Instrument {
    id: Uuid,
    name: String,
    family_id: Uuid,
    family: String,
    transposition: Option<String>,
    aliases: Vec<String>,
    sort: i32,
    /// Parts and member assignments that use the instrument.
    usage: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstrumentInput {
    name: String,
    family_id: Uuid,
    transposition: Option<String>,
    #[serde(default)]
    aliases: Vec<String>,
    sort: Option<i32>,
}

impl InstrumentInput {
    fn clean(mut self) -> AppResult<Self> {
        self.name = clean_name(&self.name)?;
        self.transposition = self.transposition.map(|t| t.trim().to_string()).filter(|t| !t.is_empty());
        if self.transposition.as_ref().is_some_and(|t| t.chars().count() > 10) {
            return Err(AppError::Invalid("a transposition has at most 10 characters".into()));
        }
        let mut aliases: Vec<String> = Vec::new();
        for alias in self.aliases {
            let alias = alias.trim().to_string();
            if !alias.is_empty()
                && !alias.eq_ignore_ascii_case(&self.name)
                && !aliases.iter().any(|a| a.eq_ignore_ascii_case(&alias))
            {
                aliases.push(alias);
            }
        }
        if aliases.len() > 30 || aliases.iter().any(|a| a.chars().count() > 80) {
            return Err(AppError::Invalid("at most 30 aliases of up to 80 characters".into()));
        }
        self.aliases = aliases;
        Ok(self)
    }
}

async fn fetch_instrument(pool: &sqlx::PgPool, organization_id: Uuid, id: Uuid) -> AppResult<Instrument> {
    sqlx::query_as!(
        Instrument,
        r#"SELECT i.id, i.name, f.id AS family_id, f.name AS family, i.transposition, i.aliases, i.sort,
                  (SELECT count(*) FROM parts p WHERE p.instrument_id = i.id)
                  + (SELECT count(*) FROM ensemble_member_parts m WHERE m.instrument_id = i.id) AS "usage!"
           FROM instruments i JOIN instrument_families f ON f.id = i.family_id
           WHERE i.id = $1 AND i.organization_id = $2"#,
        id,
        organization_id
    )
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)
}

pub async fn list_instruments(state: &App, user: &CurrentUser) -> AppResult<Vec<Instrument>> {
    let instruments = sqlx::query_as!(
        Instrument,
        r#"SELECT i.id, i.name, f.id AS family_id, f.name AS family, i.transposition, i.aliases, i.sort,
                  (SELECT count(*) FROM parts p WHERE p.instrument_id = i.id)
                  + (SELECT count(*) FROM ensemble_member_parts m WHERE m.instrument_id = i.id) AS "usage!"
           FROM instruments i JOIN instrument_families f ON f.id = i.family_id
           WHERE i.organization_id = $1
           ORDER BY f.sort, lower(f.name), i.sort, lower(i.name)"#,
        user.organization_id
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(instruments)
}

async fn check_family(pool: &sqlx::PgPool, organization_id: Uuid, family_id: Uuid) -> AppResult<()> {
    let known = sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM instrument_families WHERE id = $1 AND organization_id = $2) AS "known!""#,
        family_id,
        organization_id
    )
    .fetch_one(pool)
    .await?;
    if !known {
        return Err(AppError::Invalid("unknown family".into()));
    }
    Ok(())
}

pub async fn create_instrument(state: &App, user: &CurrentUser, input: InstrumentInput) -> AppResult<Instrument> {
    user.require(user.is_staff())?;
    let input = input.clean()?;
    check_family(&state.pool, user.organization_id, input.family_id).await?;
    let id = sqlx::query_scalar!(
        "INSERT INTO instruments (organization_id, name, family_id, transposition, aliases, sort)
         VALUES ($1, $2, $3, $4, $5, coalesce($6, (SELECT coalesce(max(sort), 0) + 1 FROM instruments
                                                   WHERE family_id = $3)))
         RETURNING id",
        user.organization_id,
        input.name,
        input.family_id,
        input.transposition,
        &input.aliases,
        input.sort
    )
    .fetch_one(&state.pool)
    .await
    .map_err(|e| unique_violation(e, "an instrument with this name exists"))?;
    fetch_instrument(&state.pool, user.organization_id, id).await
}

pub async fn update_instrument(
    state: &App,
    user: &CurrentUser,
    instrument_id: Uuid,
    input: InstrumentInput,
) -> AppResult<Instrument> {
    user.require(user.is_staff())?;
    let input = input.clean()?;
    check_family(&state.pool, user.organization_id, input.family_id).await?;
    let updated = sqlx::query!(
        "UPDATE instruments SET name = $3, family_id = $4, transposition = $5, aliases = $6, sort = coalesce($7, sort)
         WHERE id = $1 AND organization_id = $2",
        instrument_id,
        user.organization_id,
        input.name,
        input.family_id,
        input.transposition,
        &input.aliases,
        input.sort
    )
    .execute(&state.pool)
    .await
    .map_err(|e| unique_violation(e, "an instrument with this name exists"))?;
    if updated.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    fetch_instrument(&state.pool, user.organization_id, instrument_id).await
}

/// Only unused instruments go: removing one would take parts off members' stands without warning.
pub async fn delete_instrument(state: &App, user: &CurrentUser, instrument_id: Uuid) -> AppResult<()> {
    user.require(user.is_staff())?;
    let instrument = fetch_instrument(&state.pool, user.organization_id, instrument_id).await?;
    if instrument.usage > 0 {
        return Err(AppError::Conflict("the instrument is still used by parts or members".into()));
    }
    sqlx::query!("DELETE FROM instruments WHERE id = $1", instrument_id).execute(&state.pool).await?;
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MergeInput {
    /// The instrument that stays.
    into: Uuid,
}

/// Folds one instrument into another: parts, members' assignments and recordings move over, the name and other
/// names become other names of the one that stays (so file names and imports keep being recognised), and the
/// instrument goes.
pub async fn merge_instrument(
    state: &App,
    user: &CurrentUser,
    instrument_id: Uuid,
    input: MergeInput,
) -> AppResult<Instrument> {
    user.require(user.is_staff())?;
    if input.into == instrument_id {
        return Err(AppError::Invalid("an instrument cannot be merged into itself".into()));
    }
    let gone = fetch_instrument(&state.pool, user.organization_id, instrument_id).await?;
    let stays = fetch_instrument(&state.pool, user.organization_id, input.into).await?;
    let mut aliases = stays.aliases.clone();
    for name in std::iter::once(&gone.name).chain(&gone.aliases) {
        if !name.eq_ignore_ascii_case(&stays.name) && !aliases.iter().any(|a| a.eq_ignore_ascii_case(name)) {
            aliases.push(name.clone());
        }
    }
    aliases.truncate(30);
    let mut tx = state.pool.begin().await?;
    sqlx::query!("UPDATE parts SET instrument_id = $2 WHERE instrument_id = $1", instrument_id, input.into)
        .execute(&mut *tx)
        .await?;
    // Members playing both keep one assignment per part number.
    sqlx::query!(
        "DELETE FROM ensemble_member_parts a USING ensemble_member_parts b
         WHERE a.instrument_id = $1 AND b.instrument_id = $2 AND a.ensemble_id = b.ensemble_id
           AND a.user_id = b.user_id AND coalesce(a.voice, 0) = coalesce(b.voice, 0)",
        instrument_id,
        input.into
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!(
        "UPDATE ensemble_member_parts SET instrument_id = $2 WHERE instrument_id = $1",
        instrument_id,
        input.into
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!("UPDATE recordings SET instrument_id = $2 WHERE instrument_id = $1", instrument_id, input.into)
        .execute(&mut *tx)
        .await?;
    sqlx::query!("DELETE FROM instruments WHERE id = $1", instrument_id).execute(&mut *tx).await?;
    sqlx::query!("UPDATE instruments SET aliases = $2 WHERE id = $1", input.into, &aliases).execute(&mut *tx).await?;
    tx.commit().await?;
    fetch_instrument(&state.pool, user.organization_id, input.into).await
}
