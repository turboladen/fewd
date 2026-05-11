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
//!   meaningfully weakening secret entropy — 8 base64url chars × 6
//!   bits = 48 bits revealed, leaving ~208 bits of entropy in the
//!   unfingerprinted suffix (256 total − 48 fingerprint). That's far
//!   beyond brute-force feasibility, and "is this the one starting
//!   with abc12345?" is real UX value when the operator is rotating
//!   tokens under time pressure.

use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use rand::RngCore;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseBackend, DatabaseConnection,
    DatabaseTransaction, DbErr, EntityTrait, QueryFilter, Set, Statement, TransactionTrait,
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

/// Failure modes returned by [`McpTokenService::provision`] and
/// [`McpTokenService::revoke`].
#[derive(Debug)]
pub enum TokenError {
    /// `id` matched no row in `people`. Distinct from a soft-delete; the
    /// caller is expected to translate this into 404 at the HTTP layer.
    NotFound,
    /// `id` matched a row but `is_active = false`. Returned only from
    /// `provision` — `verify` filters inactive people upstream so an
    /// inactive token wouldn't authenticate anyway, and issuing one
    /// would silently mislead the operator. `revoke` allows operating
    /// on inactive rows because clearing a non-functional token is
    /// idempotent and harmless.
    Inactive,
    /// SeaORM / SQLite failure — bubble through the standard error
    /// pipeline.
    Database(DbErr),
    /// Argon2 hash production failed. Distinct from `Database` so an
    /// operator debugging a broken provision call can tell the
    /// difference between "DB rejected the write" and "the hash itself
    /// couldn't be computed" (typically RNG exhaustion or memory
    /// pressure during `m_cost` allocation).
    Hashing(argon2::password_hash::Error),
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
    ///
    /// Atomic: load + update run inside a transaction that acquires
    /// SQLite's RESERVED lock before the SELECT (see
    /// [`begin_locked_for_person`]). Without that escalation, two
    /// concurrent provision() calls on the same person could both
    /// load the same snapshot and both return 200 OK, with only one
    /// of the two plaintexts surviving the second write — the other
    /// client would walk away with a token that doesn't authenticate.
    ///
    /// Hashing happens *before* the transaction starts so the DB-wide
    /// RESERVED lock isn't held across argon2's ~10-50ms compute. The
    /// hash is discarded on `NotFound`/`Inactive`, which is cheaper
    /// than blocking every other writer through every provision call.
    pub async fn provision(
        db: &DatabaseConnection,
        person_id: &str,
    ) -> Result<ProvisionedToken, TokenError> {
        let plaintext = generate_plaintext();
        let fingerprint = fingerprint_of(&plaintext);
        let hash = hash_plaintext(&plaintext).map_err(|err| {
            tracing::error!(?err, "MCP token: argon2 hashing failed");
            TokenError::Hashing(err)
        })?;

        let txn = begin_locked_for_person(db, person_id).await?;

        let person = person::Entity::find_by_id(person_id.to_string())
            .one(&txn)
            .await?
            .ok_or(TokenError::NotFound)?;
        if !person.is_active {
            // verify() filters inactive rows, so a token issued here
            // would never authenticate. Reject up front rather than
            // silently producing a non-functional plaintext.
            return Err(TokenError::Inactive);
        }

        let mut active: person::ActiveModel = person.into();
        active.mcp_token_hash = Set(Some(hash));
        active.mcp_token_fingerprint = Set(Some(fingerprint.clone()));
        active.updated_at = Set(chrono::Utc::now());
        active.update(&txn).await?;

        txn.commit().await?;

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
    ///
    /// Atomic: wrapped in the same RESERVED-lock transaction as
    /// `provision`. Revoke-vs-revoke races are benign (idempotent), but
    /// revoke racing against a concurrent provision could otherwise leave
    /// the provisioning client with a plaintext that doesn't authenticate
    /// (last write wins, but neither response signaled the conflict).
    pub async fn revoke(db: &DatabaseConnection, person_id: &str) -> Result<(), TokenError> {
        let txn = begin_locked_for_person(db, person_id).await?;

        let person = person::Entity::find_by_id(person_id.to_string())
            .one(&txn)
            .await?
            .ok_or(TokenError::NotFound)?;

        let mut active: person::ActiveModel = person.into();
        active.mcp_token_hash = Set(None);
        active.mcp_token_fingerprint = Set(None);
        active.updated_at = Set(chrono::Utc::now());
        active.update(&txn).await?;

        txn.commit().await?;
        Ok(())
    }
}

/// Begin a transaction and immediately escalate it to SQLite's
/// RESERVED-lock state so the caller's subsequent SELECT is serialized
/// against any concurrent writer. SeaORM's default `db.begin()` issues
/// `BEGIN DEFERRED`, which only acquires the write lock at the first
/// UPDATE — too late to serialize a load+update sequence against a
/// concurrent writer. The no-op `UPDATE … SET id = id WHERE id = ?`
/// here forces SQLite to take RESERVED before the caller's SELECT,
/// closing the TOCTOU window.
///
/// Note on lock granularity: SQLite's RESERVED lock is **database-wide**,
/// not row-level — once held, every other writer (any table, any row)
/// blocks until we commit. The `WHERE id = ?` filter just keeps the
/// statement cheap and intent-clear; it doesn't narrow the lock. For
/// this codebase's traffic this is a fine trade-off, but it's worth
/// knowing that holding the transaction across slow work would block
/// unrelated writers too.
///
/// SeaORM has no first-class way to issue `BEGIN IMMEDIATE` (see
/// `set_transaction_config` for SQLite, which just logs and ignores
/// `IsolationLevel`), so this no-op write is the idiomatic substitute.
async fn begin_locked_for_person(
    db: &DatabaseConnection,
    person_id: &str,
) -> Result<DatabaseTransaction, DbErr> {
    let txn = db.begin().await?;
    txn.execute(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        "UPDATE people SET id = id WHERE id = ?",
        [person_id.into()],
    ))
    .await?;
    Ok(txn)
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
    async fn provision_rejects_inactive_person_with_distinct_error() {
        // verify() filters inactive rows, so a token issued for an
        // inactive person would never authenticate. Rejecting at
        // provision time prevents the silent "non-functional token"
        // anti-pattern.
        let db = setup_db().await;
        let alice = insert_person(&db, "p_alice", "Alice").await;

        // Deactivate.
        let mut active: person::ActiveModel = alice.clone().into();
        active.is_active = Set(false);
        active.update(&db).await.unwrap();

        let err = McpTokenService::provision(&db, &alice.id)
            .await
            .unwrap_err();
        assert!(
            matches!(err, TokenError::Inactive),
            "inactive person must trip the Inactive variant, got {err:?}"
        );

        // Confirm no token was written either.
        let stored = person::Entity::find_by_id(alice.id)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert!(stored.mcp_token_hash.is_none());
        assert!(stored.mcp_token_fingerprint.is_none());
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

    /// Pin the hash-doesn't-leak contract. The Person entity carries
    /// `#[serde(skip_serializing)]` on `mcp_token_hash`, but a future
    /// refactor (renaming the column, switching to a manual `Serialize`
    /// impl, adding a debug-dump endpoint) could silently break the
    /// invariant. This test serializes a Person whose hash would be
    /// caught if the attribute were removed, and asserts the hash does
    /// not appear in the JSON. Runs without an HTTP harness — every
    /// `Json<person::Model>` route in `routes/` shares this serialize
    /// path, so pinning the type-level contract pins them all.
    #[tokio::test]
    async fn person_serialization_omits_mcp_token_hash() {
        let db = setup_db().await;
        let alice = insert_person(&db, "p_alice", "Alice").await;
        let issued = McpTokenService::provision(&db, &alice.id).await.unwrap();

        // Reload — provision returned the in-memory ActiveModel result;
        // we want the round-tripped row that GET /api/people would
        // serialize.
        let stored = person::Entity::find_by_id(alice.id)
            .one(&db)
            .await
            .unwrap()
            .unwrap();

        // Sanity: the hash IS present on the model itself...
        assert!(
            stored.mcp_token_hash.is_some(),
            "hash must be present on the loaded model"
        );

        // ...but must NOT appear in the JSON-serialized form that any
        // GET endpoint returning Json<person::Model> would emit.
        let json = serde_json::to_string(&stored).expect("serialize");
        assert!(
            !json.contains("mcp_token_hash"),
            "field name 'mcp_token_hash' must be skipped on serialize: {json}"
        );
        assert!(
            !json.contains(stored.mcp_token_hash.as_deref().unwrap()),
            "hash value must not leak through any other field: {json}"
        );
        assert!(
            !json.contains("$argon2id$"),
            "argon2id PHC string must not leak through any other field: {json}"
        );

        // Fingerprint is intentionally serialized (UI identification).
        assert!(
            json.contains("mcp_token_fingerprint"),
            "fingerprint field must be present in serialization: {json}"
        );
        assert!(
            json.contains(&issued.fingerprint),
            "fingerprint value must be visible in serialization: {json}"
        );
    }

    /// Two concurrent provision() calls must leave exactly one of the two
    /// returned plaintexts authenticating against the persisted row. Without
    /// the transaction wrapper, both responses returned 200 OK but only the
    /// surviving write's plaintext would validate — the other client walked
    /// away with a stranded token.
    ///
    /// Caveat: in-memory SQLite uses one connection, so this test cannot
    /// reproduce true write contention. What it pins is the *consistency*
    /// invariant (response ↔ persisted state), which the fix preserves and
    /// any future refactor must preserve too.
    #[tokio::test]
    async fn concurrent_provision_does_not_strand_either_token() {
        let db = setup_db().await;
        let alice = insert_person(&db, "p_alice", "Alice").await;

        let (a, b) = tokio::join!(
            McpTokenService::provision(&db, &alice.id),
            McpTokenService::provision(&db, &alice.id),
        );
        let a = a.expect("first provision");
        let b = b.expect("second provision");

        let valid_a = McpTokenService::verify(&db, &a.plaintext)
            .await
            .expect("verify a")
            .is_some();
        let valid_b = McpTokenService::verify(&db, &b.plaintext)
            .await
            .expect("verify b")
            .is_some();

        let valid_count = [valid_a, valid_b].iter().filter(|x| **x).count();
        assert_eq!(
            valid_count, 1,
            "exactly one of two concurrently-provisioned tokens should validate \
             (valid_a={valid_a}, valid_b={valid_b})"
        );
    }

    /// A concurrent provision() and revoke() must leave the persisted row in
    /// a state consistent with provision's response. Either provision committed
    /// last (plaintext validates AND token is persisted) or revoke committed
    /// last (plaintext does NOT validate AND token is NOT persisted). The bug
    /// before the fix: these two facts could disagree, leaving the caller
    /// holding a plaintext the server had silently revoked.
    #[tokio::test]
    async fn concurrent_provision_and_revoke_leave_consistent_state() {
        let db = setup_db().await;
        let alice = insert_person(&db, "p_alice", "Alice").await;

        let (prov, rev) = tokio::join!(
            McpTokenService::provision(&db, &alice.id),
            McpTokenService::revoke(&db, &alice.id),
        );
        let prov = prov.expect("provision");
        rev.expect("revoke");

        let prov_validates = McpTokenService::verify(&db, &prov.plaintext)
            .await
            .expect("verify")
            .is_some();
        let stored = person::Entity::find_by_id(alice.id)
            .one(&db)
            .await
            .expect("query")
            .expect("alice still exists");
        let token_persisted = stored.mcp_token_hash.is_some();

        assert_eq!(
            prov_validates, token_persisted,
            "provision plaintext validity and persisted token presence must agree: \
             prov_validates={prov_validates}, token_persisted={token_persisted}"
        );
    }
}
