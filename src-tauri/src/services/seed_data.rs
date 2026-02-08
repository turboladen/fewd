use chrono::NaiveDate;
use sea_orm::*;

use crate::entities::person::{self, Entity as Person};
use crate::services::to_json;

pub async fn seed_if_empty(db: &DatabaseConnection) -> Result<(), DbErr> {
    let count = Person::find().count(db).await?;
    if count > 0 {
        return Ok(());
    }

    let now = chrono::Utc::now();
    let empty_json = to_json(&Vec::<String>::new())?;

    let people = vec![
        ("Steve", "1978-05-30"),
        ("Amanda", "1980-05-31"),
        ("Vivienne", "2014-07-10"),
        ("Cleo", "2019-03-17"),
    ];

    for (name, birthdate) in people {
        let date = NaiveDate::parse_from_str(birthdate, "%Y-%m-%d")
            .map_err(|e| DbErr::Custom(format!("Invalid seed birthdate: {}", e)))?;

        let person = person::ActiveModel {
            id: Set(uuid::Uuid::new_v4().to_string()),
            name: Set(name.to_string()),
            birthdate: Set(date),
            dietary_goals: Set(None),
            dislikes: Set(empty_json.clone()),
            favorites: Set(empty_json.clone()),
            notes: Set(None),
            is_active: Set(true),
            created_at: Set(now),
            updated_at: Set(now),
        };

        person.insert(db).await?;
    }

    Ok(())
}
