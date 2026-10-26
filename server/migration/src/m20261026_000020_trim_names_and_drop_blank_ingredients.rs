use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{DatabaseBackend, Statement};
use serde_json::Value;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// Brings stored rows in line with the write rules the services enforce:
///
/// - recipe, drink recipe, and meal template names are trimmed;
/// - recipe and drink tags are trimmed, and blank tags are dropped;
/// - a drink's `servings` below 1 is set to 1, the lowest count drink writes
///   accept;
/// - in recipe and drink ingredient lists and in the ad-hoc items of meals and
///   meal templates, every name is trimmed, an entry whose own name is blank
///   is dropped, and an `or_alternative` whose name is blank is removed.
///
/// A name that is blank after trimming is stored as the empty string, because
/// a recipe or template cannot be dropped for its name. Rows that already
/// follow the rules are not rewritten, so the migration is idempotent.
/// Unparsable JSON halts the migration with the table, column, and row id.
//
// Readers such as the shopping list, the printable, and adapt_recipe assume
// the write rules hold for every stored row. Fixing old rows here keeps that
// assumption true without a per-reader patch. Values are walked as raw JSON,
// so fields this migration does not know about pass through unchanged.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        for table in ["recipes", "drink_recipes", "meal_templates"] {
            rewrite_column(db, table, "name", trim_text).await?;
        }
        for table in ["recipes", "drink_recipes"] {
            rewrite_column(db, table, "tags", clean_tags).await?;
            rewrite_column(db, table, "ingredients", clean_ingredient_list).await?;
        }
        for table in ["meals", "meal_templates"] {
            rewrite_column(db, table, "servings", clean_servings).await?;
        }
        let clamped = db
            .execute(Statement::from_string(
                DatabaseBackend::Sqlite,
                "UPDATE drink_recipes SET servings = 1 WHERE servings < 1".to_string(),
            ))
            .await?;
        // This rewrites a value a person entered, so it is reported. The
        // migration crate has no tracing dependency, hence eprintln.
        if clamped.rows_affected() > 0 {
            eprintln!(
                "migration m20261026_000020: set servings to 1 on {} drink recipe(s) stored with servings below 1",
                clamped.rows_affected()
            );
        }
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // No-op. The padding and the nameless rows carried no information
        // worth restoring.
        Ok(())
    }
}

type Rewrite = fn(&str) -> Result<Option<String>, serde_json::Error>;

async fn rewrite_column(
    db: &SchemaManagerConnection<'_>,
    table: &str,
    column: &str,
    rewrite: Rewrite,
) -> Result<(), DbErr> {
    let rows = db
        .query_all(Statement::from_string(
            DatabaseBackend::Sqlite,
            format!("SELECT id, {column} FROM {table} ORDER BY id"),
        ))
        .await?;
    for row in rows {
        let id: String = row.try_get("", "id")?;
        let Some(original) = row.try_get::<Option<String>>("", column)? else {
            continue;
        };
        let rewritten = rewrite(&original).map_err(|e| {
            DbErr::Custom(format!(
                "{table} row {id} has unprocessable {column} JSON: {e}. \
                 Fix the row manually before re-running migrations."
            ))
        })?;
        let Some(rewritten) = rewritten else {
            continue;
        };
        db.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            format!("UPDATE {table} SET {column} = ? WHERE id = ?"),
            [rewritten.into(), id.into()],
        ))
        .await?;
    }
    Ok(())
}

fn trim_text(raw: &str) -> Result<Option<String>, serde_json::Error> {
    let trimmed = raw.trim();
    Ok((trimmed != raw).then(|| trimmed.to_string()))
}

fn clean_tags(raw: &str) -> Result<Option<String>, serde_json::Error> {
    let tags: Vec<String> = serde_json::from_str(raw)?;
    let cleaned: Vec<String> = tags
        .iter()
        .map(|tag| tag.trim().to_string())
        .filter(|tag| !tag.is_empty())
        .collect();
    if cleaned == tags {
        return Ok(None);
    }
    serde_json::to_string(&cleaned).map(Some)
}

fn clean_ingredient_list(raw: &str) -> Result<Option<String>, serde_json::Error> {
    let mut list: Vec<Value> = serde_json::from_str(raw)?;
    if !clean_ingredients(&mut list) {
        return Ok(None);
    }
    serde_json::to_string(&list).map(Some)
}

fn clean_servings(raw: &str) -> Result<Option<String>, serde_json::Error> {
    let mut servings: Vec<Value> = serde_json::from_str(raw)?;
    let mut changed = false;
    for serving in &mut servings {
        if serving.get("food_type").and_then(Value::as_str) != Some("adhoc") {
            continue;
        }
        if let Some(Value::Array(items)) = serving.get_mut("adhoc_items") {
            changed |= clean_ingredients(items);
        }
    }
    if !changed {
        return Ok(None);
    }
    serde_json::to_string(&servings).map(Some)
}

// Returns whether anything changed. An entry without a string name is left
// alone, since it is not one this migration can judge.
fn clean_ingredients(list: &mut Vec<Value>) -> bool {
    let before = list.len();
    list.retain(|ingredient| name_of(ingredient).is_none_or(|name| !name.trim().is_empty()));
    let mut changed = list.len() != before;
    for ingredient in list.iter_mut() {
        changed |= clean_ingredient(ingredient);
    }
    changed
}

// Trims this entry's name and cleans its alternative chain. The caller has
// already dropped the entry if its own name is blank.
fn clean_ingredient(ingredient: &mut Value) -> bool {
    let mut changed = trim_name(ingredient);
    let Some(object) = ingredient.as_object_mut() else {
        return changed;
    };
    let blank_alternative = object
        .get("or_alternative")
        .and_then(name_of)
        .is_some_and(|name| name.trim().is_empty());
    if blank_alternative {
        object.remove("or_alternative");
        return true;
    }
    if let Some(alternative) = object.get_mut("or_alternative") {
        changed |= clean_ingredient(alternative);
    }
    changed
}

fn name_of(ingredient: &Value) -> Option<&str> {
    ingredient.get("name").and_then(Value::as_str)
}

fn trim_name(ingredient: &mut Value) -> bool {
    let Some(Value::String(name)) = ingredient.get_mut("name") else {
        return false;
    };
    let trimmed = name.trim();
    if trimmed == name {
        return false;
    }
    *name = trimmed.to_string();
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm_migration::sea_orm::{ConnectionTrait, Database, DatabaseConnection};
    use serde_json::json;

    // Only the columns the migration reads and writes. Each table matches
    // the shipped column names.
    const SCHEMA: &str = r#"
        CREATE TABLE recipes (id TEXT PRIMARY KEY, name TEXT NOT NULL,
            tags TEXT NOT NULL, ingredients TEXT NOT NULL);
        CREATE TABLE drink_recipes (id TEXT PRIMARY KEY, name TEXT NOT NULL,
            tags TEXT NOT NULL, ingredients TEXT NOT NULL,
            servings INTEGER NOT NULL DEFAULT 1);
        CREATE TABLE meals (id TEXT PRIMARY KEY, servings TEXT NOT NULL);
        CREATE TABLE meal_templates (id TEXT PRIMARY KEY, name TEXT NOT NULL,
            servings TEXT NOT NULL);
    "#;

    async fn legacy_db() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared(SCHEMA).await.unwrap();
        db
    }

    async fn run(db: &DatabaseConnection) -> Result<(), DbErr> {
        Migration.up(&SchemaManager::new(db)).await
    }

    async fn insert(db: &DatabaseConnection, table: &str, columns: &[(&str, String)]) {
        let names: Vec<&str> = columns.iter().map(|(name, _)| *name).collect();
        let marks = vec!["?"; columns.len()].join(", ");
        db.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            format!(
                "INSERT INTO {table} ({}) VALUES ({marks})",
                names.join(", ")
            ),
            columns.iter().map(|(_, value)| value.clone().into()),
        ))
        .await
        .unwrap();
    }

    async fn column(db: &DatabaseConnection, table: &str, column: &str, id: &str) -> String {
        let row = db
            .query_one(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                format!("SELECT {column} FROM {table} WHERE id = ?"),
                [id.into()],
            ))
            .await
            .unwrap()
            .unwrap();
        row.try_get("", column).unwrap()
    }

    async fn json_column(db: &DatabaseConnection, table: &str, name: &str, id: &str) -> Value {
        serde_json::from_str(&column(db, table, name, id).await).unwrap()
    }

    fn ing(name: &str) -> Value {
        json!({"name": name, "amount": {"type": "single", "value": 1.0}, "unit": "cup", "notes": null})
    }

    fn with_alt(name: &str, alternative: Value) -> Value {
        let mut ingredient = ing(name);
        ingredient["or_alternative"] = alternative;
        ingredient
    }

    fn names(list: &Value) -> Vec<String> {
        list.as_array()
            .unwrap()
            .iter()
            .map(|i| i["name"].as_str().unwrap().to_string())
            .collect()
    }

    #[tokio::test]
    async fn trims_recipe_and_drink_names_tags_and_ingredients() {
        let db = legacy_db().await;
        let ingredients = json!([
            ing(" garlic "),
            ing("  "),
            with_alt("lime", with_alt(" lemon", ing(""))),
            with_alt("", ing("rum")),
        ])
        .to_string();
        for table in ["recipes", "drink_recipes"] {
            insert(
                &db,
                table,
                &[
                    ("id", "r1".into()),
                    ("name", "  Mai Tai\t".into()),
                    ("tags", json!([" tiki ", "", "Rum"]).to_string()),
                    ("ingredients", ingredients.clone()),
                ],
            )
            .await;
        }

        run(&db).await.unwrap();

        for table in ["recipes", "drink_recipes"] {
            assert_eq!(column(&db, table, "name", "r1").await, "Mai Tai", "{table}");
            assert_eq!(
                json_column(&db, table, "tags", "r1").await,
                json!(["tiki", "Rum"]),
                "{table}"
            );
            let stored = json_column(&db, table, "ingredients", "r1").await;
            assert_eq!(names(&stored), ["garlic", "lime"], "{table}");
            let lemon = &stored[1]["or_alternative"];
            assert_eq!(lemon["name"], "lemon", "{table}");
            assert!(lemon.get("or_alternative").is_none(), "{table}: {stored}");
            assert_eq!(stored[0]["unit"], "cup", "other fields pass through");
        }
    }

    #[tokio::test]
    async fn cleans_adhoc_items_in_meals_and_templates_and_leaves_recipe_servings_alone() {
        let db = legacy_db().await;
        let servings = json!([
            {"food_type": "recipe", "person_id": "p1", "recipe_id": " r1 ", "servings_count": 1.0, "notes": null},
            {"food_type": "adhoc", "person_id": "p1", "notes": null,
             "adhoc_items": [ing(" bread"), ing("\n"), with_alt("butter", ing(" "))]},
        ])
        .to_string();
        insert(
            &db,
            "meals",
            &[("id", "m1".into()), ("servings", servings.clone())],
        )
        .await;
        insert(
            &db,
            "meal_templates",
            &[
                ("id", "t1".into()),
                ("name", " Quick Brekkie ".into()),
                ("servings", servings),
            ],
        )
        .await;

        run(&db).await.unwrap();

        assert_eq!(
            column(&db, "meal_templates", "name", "t1").await,
            "Quick Brekkie"
        );
        for (table, id) in [("meals", "m1"), ("meal_templates", "t1")] {
            let stored = json_column(&db, table, "servings", id).await;
            assert_eq!(stored[0]["recipe_id"], " r1 ", "{table}: ids are not names");
            let items = &stored[1]["adhoc_items"];
            assert_eq!(names(items), ["bread", "butter"], "{table}");
            assert!(items[1].get("or_alternative").is_none(), "{table}: {items}");
        }
    }

    #[tokio::test]
    async fn clean_rows_are_left_byte_for_byte_and_a_rerun_changes_nothing() {
        let db = legacy_db().await;
        // Key order and spacing that serde_json would not reproduce.
        let clean =
            r#"[{"unit": "cup", "name": "garlic", "amount": {"type": "single", "value": 1}}]"#;
        insert(
            &db,
            "recipes",
            &[
                ("id", "clean".into()),
                ("name", "Pasta".into()),
                ("tags", r#"["dinner"]"#.into()),
                ("ingredients", clean.into()),
            ],
        )
        .await;
        insert(
            &db,
            "recipes",
            &[
                ("id", "padded".into()),
                ("name", " Soup".into()),
                ("tags", r#"[" lunch"]"#.into()),
                ("ingredients", json!([ing(" leek")]).to_string()),
            ],
        )
        .await;

        run(&db).await.unwrap();
        let after_first = column(&db, "recipes", "ingredients", "padded").await;
        run(&db).await.unwrap();

        assert_eq!(column(&db, "recipes", "ingredients", "clean").await, clean);
        assert_eq!(
            column(&db, "recipes", "ingredients", "padded").await,
            after_first
        );
        assert_eq!(column(&db, "recipes", "name", "padded").await, "Soup");
    }

    #[tokio::test]
    async fn clamps_drink_servings_below_one_and_leaves_the_rest() {
        let db = legacy_db().await;
        for (id, servings) in [("zero", "0"), ("negative", "-2"), ("two", "2")] {
            insert(
                &db,
                "drink_recipes",
                &[
                    ("id", id.into()),
                    ("name", "Negroni".into()),
                    ("tags", "[]".into()),
                    ("ingredients", "[]".into()),
                    ("servings", servings.into()),
                ],
            )
            .await;
        }

        run(&db).await.unwrap();

        for (id, expected) in [("zero", 1), ("negative", 1), ("two", 2)] {
            let row = db
                .query_one(Statement::from_sql_and_values(
                    DatabaseBackend::Sqlite,
                    "SELECT servings FROM drink_recipes WHERE id = ?".to_string(),
                    [id.into()],
                ))
                .await
                .unwrap()
                .unwrap();
            let servings: i32 = row.try_get("", "servings").unwrap();
            assert_eq!(servings, expected, "{id}");
        }
    }

    #[tokio::test]
    async fn unknown_keys_pass_through_a_rewritten_row_at_every_depth() {
        let db = legacy_db().await;
        let mut alternative = ing(" lemon");
        alternative["x_alt"] = json!({"kept": [1, 2]});
        let mut ingredient = with_alt(" lime", alternative);
        ingredient["x_row"] = json!("kept");
        let servings = json!([{
            "food_type": "adhoc", "person_id": "p1", "notes": null, "x_serving": true,
            "adhoc_items": [ingredient.clone()],
        }]);
        insert(
            &db,
            "recipes",
            &[
                ("id", "r1".into()),
                ("name", "Mojito".into()),
                ("tags", "[]".into()),
                ("ingredients", json!([ingredient]).to_string()),
            ],
        )
        .await;
        insert(
            &db,
            "meals",
            &[("id", "m1".into()), ("servings", servings.to_string())],
        )
        .await;

        run(&db).await.unwrap();

        let stored = json_column(&db, "recipes", "ingredients", "r1").await;
        let meal = json_column(&db, "meals", "servings", "m1").await;
        assert_eq!(meal[0]["x_serving"], true, "{meal}");
        for row in [&stored[0], &meal[0]["adhoc_items"][0]] {
            assert_eq!(row["name"], "lime", "{row}");
            assert_eq!(row["x_row"], "kept", "{row}");
            assert_eq!(row["or_alternative"]["name"], "lemon", "{row}");
            assert_eq!(
                row["or_alternative"]["x_alt"],
                json!({"kept": [1, 2]}),
                "{row}"
            );
        }
    }

    #[tokio::test]
    async fn a_blank_recipe_name_is_stored_empty() {
        let db = legacy_db().await;
        insert(
            &db,
            "recipes",
            &[
                ("id", "r1".into()),
                ("name", "   ".into()),
                ("tags", "[]".into()),
                ("ingredients", "[]".into()),
            ],
        )
        .await;
        run(&db).await.unwrap();
        assert_eq!(column(&db, "recipes", "name", "r1").await, "");
    }

    #[tokio::test]
    async fn corrupt_json_halts_the_migration_naming_the_row() {
        let db = legacy_db().await;
        insert(
            &db,
            "meals",
            &[("id", "m1".into()), ("servings", "{ not json".into())],
        )
        .await;
        let msg = run(&db).await.unwrap_err().to_string();
        assert!(msg.contains("meals row m1"), "{msg}");
        assert!(msg.contains("servings JSON"), "{msg}");
    }
}
