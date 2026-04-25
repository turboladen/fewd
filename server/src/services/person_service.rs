use chrono::NaiveDate;
use sea_orm::*;

use crate::dto::{CreatePersonDto, UpdatePersonDto};
use crate::entities::person::{self, Entity as Person};
use crate::services::to_json;

pub struct PersonService;

impl PersonService {
    pub async fn get_all(db: &DatabaseConnection) -> Result<Vec<person::Model>, DbErr> {
        Person::find()
            .filter(person::Column::IsActive.eq(true))
            .order_by_asc(person::Column::Name)
            .all(db)
            .await
    }

    pub async fn get_by_id(
        db: &DatabaseConnection,
        id: String,
    ) -> Result<Option<person::Model>, DbErr> {
        Person::find_by_id(id).one(db).await
    }

    /// Case-insensitive lookup of an active family member by name. Used by the
    /// MCP server's light "family-name bearer" auth. The active set is small
    /// (a handful of rows) so we filter in-memory rather than fight SQLite's
    /// case-sensitivity rules at the query layer.
    ///
    /// Returns `Ok(None)` if no row matches OR if multiple active rows
    /// normalize to the same name. The latter shouldn't happen — there's no
    /// uniqueness constraint enforcing it today, but the household-scale data
    /// model makes it very unlikely. Failing closed on ambiguity (rather than
    /// silently picking the first match) avoids "auth resolves as the wrong
    /// person" scenarios; the accompanying `tracing::warn!` makes the
    /// duplicate visible so the operator can clean it up.
    pub async fn find_active_by_name(
        db: &DatabaseConnection,
        name: &str,
    ) -> Result<Option<person::Model>, DbErr> {
        let target = name.trim().to_lowercase();
        if target.is_empty() {
            return Ok(None);
        }
        let matches: Vec<person::Model> = Self::get_all(db)
            .await?
            .into_iter()
            .filter(|p| p.name.trim().to_lowercase() == target)
            .collect();
        match matches.len() {
            0 => Ok(None),
            1 => Ok(matches.into_iter().next()),
            n => {
                let ids: Vec<&str> = matches.iter().map(|p| p.id.as_str()).collect();
                tracing::warn!(
                    normalized_name = %target,
                    matched_ids = ?ids,
                    match_count = n,
                    "find_active_by_name: multiple active people normalize to the same name; \
                     refusing to disambiguate. Edit one of the duplicates so they're distinguishable."
                );
                Ok(None)
            }
        }
    }

    pub async fn create(
        db: &DatabaseConnection,
        data: CreatePersonDto,
    ) -> Result<person::Model, DbErr> {
        let now = chrono::Utc::now();
        let birthdate = NaiveDate::parse_from_str(&data.birthdate, "%Y-%m-%d")
            .map_err(|e| DbErr::Custom(format!("Invalid birthdate format: {}", e)))?;

        let person = person::ActiveModel {
            id: Set(uuid::Uuid::new_v4().to_string()),
            name: Set(data.name),
            birthdate: Set(birthdate),
            dietary_goals: Set(data.dietary_goals),
            dislikes: Set(to_json(&data.dislikes)?),
            favorites: Set(to_json(&data.favorites)?),
            notes: Set(data.notes),
            drink_preferences: Set(data.drink_preferences.map(|v| to_json(&v)).transpose()?),
            drink_dislikes: Set(data.drink_dislikes.map(|v| to_json(&v)).transpose()?),
            is_active: Set(true),
            created_at: Set(now),
            updated_at: Set(now),
        };

        person.insert(db).await
    }

    pub async fn update(
        db: &DatabaseConnection,
        id: String,
        data: UpdatePersonDto,
    ) -> Result<person::Model, DbErr> {
        let existing = Person::find_by_id(id)
            .one(db)
            .await?
            .ok_or(DbErr::RecordNotFound("Person not found".to_string()))?;

        let mut person: person::ActiveModel = existing.into();

        if let Some(name) = data.name {
            person.name = Set(name);
        }
        if let Some(birthdate) = data.birthdate {
            let parsed = NaiveDate::parse_from_str(&birthdate, "%Y-%m-%d")
                .map_err(|e| DbErr::Custom(format!("Invalid birthdate format: {}", e)))?;
            person.birthdate = Set(parsed);
        }
        if let Some(dietary_goals) = data.dietary_goals {
            person.dietary_goals = Set(Some(dietary_goals));
        }
        if let Some(dislikes) = data.dislikes {
            person.dislikes = Set(to_json(&dislikes)?);
        }
        if let Some(favorites) = data.favorites {
            person.favorites = Set(to_json(&favorites)?);
        }
        if let Some(notes) = data.notes {
            person.notes = Set(Some(notes));
        }
        if let Some(is_active) = data.is_active {
            person.is_active = Set(is_active);
        }
        if let Some(drink_preferences) = data.drink_preferences {
            person.drink_preferences = Set(Some(to_json(&drink_preferences)?));
        }
        if let Some(drink_dislikes) = data.drink_dislikes {
            person.drink_dislikes = Set(Some(to_json(&drink_dislikes)?));
        }

        person.updated_at = Set(chrono::Utc::now());

        person.update(db).await
    }

    pub async fn delete(db: &DatabaseConnection, id: String) -> Result<(), DbErr> {
        Person::delete_by_id(id).exec(db).await?;
        Ok(())
    }
}
