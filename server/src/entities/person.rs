use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "people")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub name: String,
    pub birthdate: Date,
    pub dietary_goals: Option<String>,
    #[sea_orm(column_type = "Text")]
    pub dislikes: String,
    #[sea_orm(column_type = "Text")]
    pub favorites: String,
    pub notes: Option<String>,
    #[sea_orm(column_type = "Text")]
    pub drink_preferences: Option<String>,
    #[sea_orm(column_type = "Text")]
    pub drink_dislikes: Option<String>,
    pub is_active: bool,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
    /// Argon2id hash of the per-person MCP bearer token. `None` until the
    /// token is provisioned via the web Settings UI; once present, the
    /// matching plaintext token authenticates this person to `/mcp`. The
    /// plaintext is never stored — only the hash. Skipped on serialization
    /// so the hash never leaks through any GET endpoint.
    #[serde(skip_serializing)]
    #[sea_orm(column_type = "Text")]
    pub mcp_token_hash: Option<String>,
    /// First 8 base64url chars of the plaintext token. Stored cleartext
    /// because it intentionally identifies which token this is — in the
    /// Settings UI ("Token starts with abc12345…") and in operator logs
    /// when revoking a leaked token. 8 chars × 6 bits = 48 bits exposed,
    /// leaving ~208 bits of entropy in the unfingerprinted suffix
    /// (256 total − 48 fingerprint), well beyond brute-force feasibility.
    #[sea_orm(column_type = "Text")]
    pub mcp_token_fingerprint: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
