use sea_orm::*;

use crate::dto::{BulkBarItemsDto, CreateBarItemDto};
use crate::entities::bar_item::{self, Entity as BarItem};

pub struct BarItemService;

impl BarItemService {
    pub async fn get_all(db: &DatabaseConnection) -> Result<Vec<bar_item::Model>, DbErr> {
        BarItem::find()
            .order_by_asc(bar_item::Column::Category)
            .order_by_asc(bar_item::Column::Name)
            .all(db)
            .await
    }

    pub async fn create(
        db: &DatabaseConnection,
        data: CreateBarItemDto,
    ) -> Result<bar_item::Model, DbErr> {
        let item = bar_item::ActiveModel {
            id: Set(uuid::Uuid::new_v4().to_string()),
            name: Set(data.name),
            category: Set(data.category),
            created_at: Set(chrono::Utc::now()),
        };

        item.insert(db).await
    }

    pub async fn bulk_create(
        db: &DatabaseConnection,
        data: BulkBarItemsDto,
    ) -> Result<Vec<bar_item::Model>, DbErr> {
        let now = chrono::Utc::now();
        let mut created = Vec::with_capacity(data.items.len());

        for item_dto in data.items {
            let item = bar_item::ActiveModel {
                id: Set(uuid::Uuid::new_v4().to_string()),
                name: Set(item_dto.name),
                category: Set(item_dto.category),
                created_at: Set(now),
            };
            created.push(item.insert(db).await?);
        }

        Ok(created)
    }

    pub async fn delete(db: &DatabaseConnection, id: String) -> Result<(), DbErr> {
        BarItem::delete_by_id(id).exec(db).await?;
        Ok(())
    }

    pub async fn delete_all(db: &DatabaseConnection) -> Result<u64, DbErr> {
        let result = BarItem::delete_many().exec(db).await?;
        Ok(result.rows_affected)
    }
}
