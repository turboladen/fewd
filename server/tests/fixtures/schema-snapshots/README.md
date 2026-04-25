# Schema snapshots

`baseline.sql` represents the schema state of the most recently deployed
version of the server. The migration smoke test
(`scripts/migration-smoke-test.sh`) loads it into a fresh SQLite DB and
boots the **release build** of `fewd-server` against it, which exercises
two failure modes that ordinary `cargo test` misses:

1. **In-place edits to already-applied migrations.** SeaORM tracks migrations
   by name in `seaql_migrations`. If someone edits an already-applied
   migration to add a column or index, existing DBs (anywhere the original
   has run) silently skip the modified version. The baseline contains the
   `seaql_migrations` rows for every migration in the deployed snapshot, so
   when the server boots against it those migrations are skipped — and any
   schema change buried inside them goes unapplied. Subsequent queries
   (`/api/recipes`, `/api/drink-recipes`, `/api/meals`) then 500, which the
   script catches.

2. **Release-only feature-flag panics.** `cargo test --all-features` unifies
   dev-dependency features and can hide runtime feature gaps in
   `sea-orm-migration` helpers. The smoke test runs the release binary, so
   any `"Sqlite feature is off"`-style panic from gating like
   `SchemaManager::has_column()` surfaces immediately.

The script also runs a second case against an empty DB (the fresh-install
path) to catch new migrations that work in tests but panic in release
builds.

## Contents

- `CREATE TABLE` / `CREATE INDEX` statements for every table the server
  expects to find at boot
- `INSERT` rows for `seaql_migrations` so SeaORM treats every shipped
  migration as already applied
- **No application data.** `seed_if_empty` runs at server boot and
  populates seed people; carrying seed rows here would mask future seed
  regressions because the seed function is keyed off
  `Person::find().count() > 0`.

## When to regenerate

Regenerate `baseline.sql` after every successful deploy. The fixture is
meant to mirror "what is currently live on dietpi" — between deploys it
stays put so any in-place migration edit introduced during that window
gets caught.

If you forget to regenerate after a deploy, the smoke test will fail loudly
on case A's `up()` for any new migration added since the last regen. That
is a feature: it forces a deliberate confirmation that the new migration is
safe against existing prod state before regenerating.

## How to regenerate

From the project root:

```bash
just db-reset
{
  sqlite3 server/data/fewd.db .schema
  sqlite3 server/data/fewd.db \
    "SELECT 'INSERT INTO seaql_migrations VALUES(' || quote(version) || ',' || applied_at || ');' FROM seaql_migrations;"
} > server/tests/fixtures/schema-snapshots/baseline.sql
```

`just db-reset` deletes `server/data/fewd.db` and reruns all migrations
from scratch. The two `sqlite3` invocations dump:

1. All `CREATE` statements (schema only — no data)
2. One `INSERT` per migration so SeaORM treats them as applied

Then run `just smoke-test` to confirm the new fixture works end-to-end
before committing.
