use diesel::deserialize::QueryableByName;
use diesel::sql_types::Text;
use diesel::{sql_query, PgConnection, RunQueryDsl};

#[derive(QueryableByName, Debug)]
struct ConstraintName {
    #[diesel(sql_type = Text)]
    conname: String,
}

/// Return the unique constraints for a table.
///
/// # Arguments
///
/// * `conn` - the connection to the database
/// * `table_name` - the name of the table
pub fn get_unique_constraints(
    conn: &mut PgConnection,
    table_name: &str,
) -> Result<Vec<String>, diesel::result::Error> {
    // 'u' = unique constraint
    // ::regclass returns the table's object ID
    let query = format!(
        "SELECT conname FROM pg_constraint WHERE conrelid = '{}'::regclass AND contype = 'u'",
        table_name
    );

    sql_query(query)
        .load::<ConstraintName>(conn)
        .map(|constraints| {
            constraints
                .into_iter()
                .map(|constraint| constraint.conname)
                .collect()
        })
}
