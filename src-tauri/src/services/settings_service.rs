use sea_orm::*;

use crate::entities::setting::{self, Entity as Setting};

pub struct SettingsService;

impl SettingsService {
    pub async fn get(db: &DatabaseConnection, key: String) -> Result<Option<String>, DbErr> {
        let result = Setting::find_by_id(key).one(db).await?;
        Ok(result.map(|m| m.value))
    }

    /// Upsert: insert if key doesn't exist, update if it does
    pub async fn set(db: &DatabaseConnection, key: String, value: String) -> Result<(), DbErr> {
        let existing = Setting::find_by_id(key.clone()).one(db).await?;

        match existing {
            Some(model) => {
                let mut active: setting::ActiveModel = model.into();
                active.value = Set(value);
                active.update(db).await?;
            }
            None => {
                let new_setting = setting::ActiveModel {
                    key: Set(key),
                    value: Set(value),
                };
                new_setting.insert(db).await?;
            }
        }

        Ok(())
    }

    pub async fn delete(db: &DatabaseConnection, key: String) -> Result<(), DbErr> {
        Setting::delete_by_id(key).exec(db).await?;
        Ok(())
    }
}
