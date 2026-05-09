//! MCP per-person token issuance, verification, and revocation (fewd-2y6.6).
//!
//! # Threat model
//!
//! - Tokens are 32 random bytes (256 bits) from the OS CSPRNG, base64url-
//!   encoded for transport (43 ASCII chars, no padding). The plaintext is
//!   shown to the user exactly once at provision time, never stored, and
//!   never logged.
//! - At rest we store an Argon2id hash with library defaults — slow
//!   verification is acceptable here because MCP requests are
//!   non-interactive and the hash check happens once per HTTP request,
//!   not in a tight loop.
//! - Token comparison goes through `argon2`'s built-in `verify_password`
//!   which does the constant-time check internally; we never `==` raw
//!   plaintext or hashes.
//! - The fingerprint (first 8 plaintext chars) is stored cleartext on
//!   purpose. It identifies which token a UI row represents without
//!   leaking secret entropy — 24 effective bits (8 base64url chars × 6
//!   bits — minus a tiny bias from the URL alphabet) is fine for human
//!   "is this the one starting with abc12345?" without weakening the
//!   210+ bits of entropy in the unfingerprinted suffix.

use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use rand::RngCore;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter, Set,
};

use crate::entities::person;

/// Result of provisioning a fresh MCP token for a person. The plaintext
/// must be displayed to the user exactly once and discarded — it is not
/// recoverable from `mcp_token_hash` afterward.
#[derive(Debug)]
pub struct ProvisionedToken {
    /// The bearer string the client should send as
    /// `Authorization: Bearer <plaintext>`. Shown once, then thrown
    /// away — the server only retains the hash + fingerprint.
    pub plaintext: String,
    /// First 8 chars of the plaintext, persisted alongside the hash for
    /// UI identification ("Token starts with abc12345…").
    pub fingerprint: String,
}

/// Failure modes returned by [`provision`] and [`revoke`].
#[derive(Debug)]
pub enum TokenError {
    /// `id` matched no row in `people`. Distinct from a soft-delete; the
    /// caller is expected to translate this into 404 at the HTTP layer.
    NotFound,
    /// SeaORM / SQLite failure — bubble through the standard error
    /// pipeline. Argon2 hashing failures also collapse here because they
    /// indicate a process-level problem (OOM, RNG exhaustion), not a
    /// client-correctable input.
    Database(DbErr),
}

impl From<DbErr> for TokenError {
    fn from(e: DbErr) -> Self {
        Self::Database(e)
    }
}

pub struct McpTokenService;

impl McpTokenService {
    /// Issue a fresh token for `person_id`, persist its argon2 hash +
    /// fingerprint, and return the plaintext for one-time display.
    /// Replaces any previously-issued token on the same row (rotation).
    pub async fn provision(
        db: &DatabaseConnection,
        person_id: &str,
    ) -> Result<ProvisionedToken, TokenError> {
        let person = person::Entity::find_by_id(person_id.to_string())
            .one(db)
            .await?
            .ok_or(TokenError::NotFound)?;

        let plaintext = generate_plaintext();
        let fingerprint = fingerprint_of(&plaintext);
        let hash = hash_plaintext(&plaintext).map_err(|err| {
            tracing::error!(?err, "MCP token: argon2 hashing failed");
            TokenError::Database(DbErr::Custom("token hash failed".to_string()))
        })?;

        let mut active: person::ActiveModel = person.into();
        active.mcp_token_hash = Set(Some(hash));
        active.mcp_token_fingerprint = Set(Some(fingerprint.clone()));
        active.update(db).await?;

        Ok(ProvisionedToken {
            plaintext,
            fingerprint,
        })
    }

    /// Resolve a presented bearer token to the matching active person, or
    /// `Ok(None)` if no active row hashes to it. Constant-time comparison
    /// is enforced by argon2's `verify_password`.
    ///
    /// Implementation: load all active people whose `mcp_token_hash` is
    /// non-null, then verify against each. At household scale (≤ a few
    /// dozen rows) the linear scan is acceptable; if this ever grows,
    /// store a non-secret token-prefix index column and filter on it
    /// before verifying.
    pub async fn verify(
        db: &DatabaseConnection,
        presented: &str,
    ) -> Result<Option<person::Model>, DbErr> {
        let candidates = person::Entity::find()
            .filter(person::Column::IsActive.eq(true))
            .filter(person::Column::McpTokenHash.is_not_null())
            .all(db)
            .await?;

        let argon2 = Argon2::default();
        for candidate in candidates {
            let Some(hash_str) = candidate.mcp_token_hash.as_deref() else {
                continue;
            };
            let parsed = match PasswordHash::new(hash_str) {
                Ok(h) => h,
                Err(err) => {
                    tracing::error!(
                        person_id = %candidate.id,
                        ?err,
                        "MCP token: stored hash did not parse — skipping"
                    );
                    continue;
                }
            };
            if argon2
                .verify_password(presented.as_bytes(), &parsed)
                .is_ok()
            {
                return Ok(Some(candidate));
            }
        }
        Ok(None)
    }

    /// Null out the hash + fingerprint columns for `person_id`. After
    /// revoke, `verify` will reject any token previously issued for this
    /// row, including the plaintext the user copied at provision time.
    pub async fn revoke(db: &DatabaseConnection, person_id: &str) -> Result<(), TokenError> {
        let person = person::Entity::find_by_id(person_id.to_string())
            .one(db)
            .await?
            .ok_or(TokenError::NotFound)?;

        let mut active: person::ActiveModel = person.into();
        active.mcp_token_hash = Set(None);
        active.mcp_token_fingerprint = Set(None);
        active.update(db).await?;
        Ok(())
    }
}

fn generate_plaintext() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

fn fingerprint_of(plaintext: &str) -> String {
    plaintext.chars().take(8).collect()
}

fn hash_plaintext(plaintext: &str) -> Result<String, argon2::password_hash::Error> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    Ok(argon2
        .hash_password(plaintext.as_bytes(), &salt)?
        .to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use migration::{Migrator, MigratorTrait};
    use sea_orm::{ActiveModelTrait, Database, Set};

    async fn setup_db() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite connects");
        Migrator::up(&db, None)
            .await
            .expect("migrations run on empty DB");
        db
    }

    async fn insert_person(db: &DatabaseConnection, id: &str, name: &str) -> person::Model {
        let active = person::ActiveModel {
            id: Set(id.to_string()),
            name: Set(name.to_string()),
            birthdate: Set(chrono::NaiveDate::from_ymd_opt(1990, 1, 1).unwrap()),
            dietary_goals: Set(None),
            dislikes: Set("[]".to_string()),
            favorites: Set("[]".to_string()),
            notes: Set(None),
            drink_preferences: Set(None),
            drink_dislikes: Set(None),
            is_active: Set(true),
            created_at: Set(chrono::Utc::now()),
            updated_at: Set(chrono::Utc::now()),
            mcp_token_hash: Set(None),
            mcp_token_fingerprint: Set(None),
        };
        active.insert(db).await.expect("insert person")
    }

    #[tokio::test]
    async fn provision_then_verify_round_trips_to_the_same_person() {
        let db = setup_db().await;
        let alice = insert_person(&db, "p_alice", "Alice").await;

        let issued = McpTokenService::provision(&db, &alice.id)
            .await
            .expect("provision");

        // Plaintext shape: 32 bytes → base64url no-pad → 43 chars
        assert_eq!(issued.plaintext.len(), 43, "plaintext length");
        assert_eq!(issued.fingerprint.len(), 8, "fingerprint length");
        assert!(
            issued.plaintext.starts_with(&issued.fingerprint),
            "fingerprint must be a prefix of the plaintext"
        );

        let resolved = McpTokenService::verify(&db, &issued.plaintext)
            .await
            .expect("verify db ok");
        let resolved = resolved.expect("verify must find the issued token");
        assert_eq!(resolved.id, alice.id);

        // The stored hash must NOT equal the plaintext.
        let stored = person::Entity::find_by_id(alice.id)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_ne!(
            stored.mcp_token_hash.as_deref(),
            Some(issued.plaintext.as_str()),
            "DB must store hash, not plaintext"
        );
        assert!(
            stored
                .mcp_token_hash
                .as_deref()
                .unwrap()
                .starts_with("$argon2id$"),
            "hash must be argon2id PHC string"
        );
        assert_eq!(
            stored.mcp_token_fingerprint.as_deref(),
            Some(issued.fingerprint.as_str())
        );
    }

    #[tokio::test]
    async fn verify_rejects_an_unrelated_token() {
        let db = setup_db().await;
        let alice = insert_person(&db, "p_alice", "Alice").await;
        let _issued = McpTokenService::provision(&db, &alice.id)
            .await
            .expect("provision");

        // Different 43-char base64url plaintext, never issued by us.
        let attacker = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
        assert_eq!(attacker.len(), 43);

        let resolved = McpTokenService::verify(&db, attacker)
            .await
            .expect("verify db ok");
        assert!(
            resolved.is_none(),
            "unrelated token must not match any stored hash"
        );
    }

    #[tokio::test]
    async fn provision_rotates_the_token_for_the_same_person() {
        let db = setup_db().await;
        let alice = insert_person(&db, "p_alice", "Alice").await;

        let first = McpTokenService::provision(&db, &alice.id).await.unwrap();
        let second = McpTokenService::provision(&db, &alice.id).await.unwrap();

        assert_ne!(
            first.plaintext, second.plaintext,
            "rotation must produce a fresh secret"
        );

        // The first token no longer authenticates.
        assert!(
            McpTokenService::verify(&db, &first.plaintext)
                .await
                .unwrap()
                .is_none(),
            "rotated-out token must be rejected"
        );
        // The current one does.
        assert!(
            McpTokenService::verify(&db, &second.plaintext)
                .await
                .unwrap()
                .is_some(),
            "current token must authenticate"
        );
    }

    #[tokio::test]
    async fn revoke_clears_columns_and_blocks_subsequent_verification() {
        let db = setup_db().await;
        let alice = insert_person(&db, "p_alice", "Alice").await;
        let issued = McpTokenService::provision(&db, &alice.id).await.unwrap();

        McpTokenService::revoke(&db, &alice.id).await.unwrap();

        let stored = person::Entity::find_by_id(alice.id.clone())
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert!(stored.mcp_token_hash.is_none());
        assert!(stored.mcp_token_fingerprint.is_none());

        let resolved = McpTokenService::verify(&db, &issued.plaintext)
            .await
            .unwrap();
        assert!(resolved.is_none(), "revoked token must not authenticate");
    }

    #[tokio::test]
    async fn provision_returns_not_found_for_unknown_id() {
        let db = setup_db().await;
        let err = McpTokenService::provision(&db, "ghost-id")
            .await
            .unwrap_err();
        assert!(matches!(err, TokenError::NotFound));
    }

    #[tokio::test]
    async fn verify_skips_inactive_people() {
        let db = setup_db().await;
        let alice = insert_person(&db, "p_alice", "Alice").await;
        let issued = McpTokenService::provision(&db, &alice.id).await.unwrap();

        // Deactivate.
        let mut active: person::ActiveModel = person::Entity::find_by_id(alice.id.clone())
            .one(&db)
            .await
            .unwrap()
            .unwrap()
            .into();
        active.is_active = Set(false);
        active.update(&db).await.unwrap();

        let resolved = McpTokenService::verify(&db, &issued.plaintext)
            .await
            .unwrap();
        assert!(
            resolved.is_none(),
            "deactivated person's token must not authenticate"
        );
    }

    #[test]
    fn fingerprint_of_takes_first_8_chars() {
        assert_eq!(fingerprint_of("ABCDEFGH_extra"), "ABCDEFGH");
        assert_eq!(fingerprint_of("short"), "short"); // edge: shorter than 8
    }
}
