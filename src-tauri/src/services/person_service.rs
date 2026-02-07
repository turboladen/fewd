use chrono::NaiveDate;
use sea_orm::*;

use crate::commands::person::{CreatePersonDto, UpdatePersonDto};
use crate::entities::person::{self, Entity as Person};

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
            dislikes: Set(serde_json::to_string(&data.dislikes).unwrap()),
            favorites: Set(serde_json::to_string(&data.favorites).unwrap()),
            notes: Set(data.notes),
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
            person.dislikes = Set(serde_json::to_string(&dislikes).unwrap());
        }
        if let Some(favorites) = data.favorites {
            person.favorites = Set(serde_json::to_string(&favorites).unwrap());
        }
        if let Some(notes) = data.notes {
            person.notes = Set(Some(notes));
        }
        if let Some(is_active) = data.is_active {
            person.is_active = Set(is_active);
        }

        person.updated_at = Set(chrono::Utc::now());

        person.update(db).await
    }

    pub async fn delete(db: &DatabaseConnection, id: String) -> Result<(), DbErr> {
        Person::delete_by_id(id).exec(db).await?;
        Ok(())
    }
}
