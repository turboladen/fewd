//! Add per-person opaque-token fields for the MCP auth surface (fewd-2y6.6).
//!
//! Both columns are nullable so the migration is purely additive — existing
//! rows are unaffected, and a fresh deployment doesn't need to provision a
//! token for every person before the system is usable. Tokens are issued
//! on-demand from the web Settings UI; until provisioned, the corresponding
//! family member simply can't authenticate to `/mcp`.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(People::Table)
                    .add_column(ColumnDef::new(People::McpTokenHash).text().null())
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(People::Table)
                    .add_column(ColumnDef::new(People::McpTokenFingerprint).text().null())
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(People::Table)
                    .drop_column(People::McpTokenFingerprint)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(People::Table)
                    .drop_column(People::McpTokenHash)
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}

#[derive(Iden)]
enum People {
    Table,
    McpTokenHash,
    McpTokenFingerprint,
}
