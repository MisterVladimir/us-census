pub mod constraints;
pub mod fetch_api_metadata;
pub mod models;
pub mod parse_geography;
pub mod parse_variables;
pub mod schema;

use crate::fetch_api_metadata::CachedClient;
use crate::fetch_api_metadata::FetchError;
use crate::models::ApiPathsGeographyAssociation;
use crate::parse_geography::{GeographyCollection, GeographyItem};
use crate::parse_variables::{VariablesCollection, VariablesItem};
use diesel::connection::DefaultLoadingMode;
use diesel::dsl::sql;
use diesel::prelude::*;
use diesel::result::Error as DieselError;
use diesel::upsert::on_constraint;
use models::{ApiPaths, ApiPathsVariablesAssociation};
use std::env;
use thiserror::Error;
use url::Url;

/// Return a database connection.
///
/// # Arguments
///
/// * `database_url` - An optional database URL. If not provided, it will try to read
///     it from the `DATABASE_URL` environment variable.
/// * `env_path` - An optional path to a `.env` file. If not provided, it will default to `.local.env`.
///
/// # Returns
///
/// * `Ok(PgConnection)` - A connection to the PostgreSQL database
/// * `Err(diesel::ConnectionError)` - The error returned by `PgConnection::establish`
///     if the connection fails
pub fn establish_database_connection(
    database_url: Option<String>,
    env_path: Option<&std::path::Path>,
) -> ConnectionResult<PgConnection> {
    let url: String = match database_url {
        Some(database_url) => database_url,
        None => {
            // Use the provided env file path or fall back to default behavior
            if let Some(path) = env_path {
                dotenvy::from_path(path).ok();
            } else {
                dotenvy::from_path(".local.env").ok();
            }
            env::var("DATABASE_URL").expect("DATABASE_URL must be set")
        }
    };
    PgConnection::establish(&url)
}

#[derive(Debug, Error)]
pub enum InsertError {
    #[error("URL parsing error: {0}")]
    UrlParse(#[from] url::ParseError),

    #[error("Database error: {0}")]
    Database(#[from] DieselError),

    #[error("JSON deserialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Error fetching API spec from web: {0}")]
    Http(#[from] FetchError), // Adjust based on your CachedClient's error type
}

/// Insert variables and geography for a given API path into the database.
///
/// # Arguments
///
/// * `conn` - connection to the datbase
/// * `client` - the client to use for fetching the API metadata (JSON)
/// * `api_path_metadata` - the API paths whose variables and geography to insert
/// * `variables_unique_key_constraint` - the unique key constraint for the variables table
pub async fn insert_variables_and_geography_for_api_path(
    conn: &mut PgConnection,
    client: &CachedClient<'_>,
    api_path_metadata: &ApiPaths<'_>,
    variables_unique_key_constraint: &str,
) -> Result<(), InsertError> {
    // Avoid exceeding the Postgres maximum number of parameters in a single query (65535).
    let safe_batch_size = 5000;

    let variables_url = Url::parse(api_path_metadata.c_variables_link.as_ref())?;
    let variables_response = client.fetch(&variables_url).await?;
    let parsed_variables_response: VariablesCollection = serde_json::from_str(&variables_response)?;

    let geography_url = Url::parse(api_path_metadata.c_geography_link.as_ref())?;
    let geography_response = client.fetch(&geography_url).await?;
    let parsed_geography_response: GeographyCollection = serde_json::from_str(&geography_response)?;

    // Use a single transaction per endpoint such that all variable and geography parameters
    // are rolled back.
    conn.transaction::<_, DieselError, _>(|conn| {
        for chunk in parsed_variables_response.variables.chunks(safe_batch_size) {
            insert_variables(
                chunk,
                conn,
                api_path_metadata.id,
                &variables_unique_key_constraint,
            )
            .map_err(|_| DieselError::RollbackTransaction)?;
        }

        for chunk in parsed_geography_response.fips.chunks(safe_batch_size) {
            insert_geographies(chunk, conn, api_path_metadata.id)
                .map_err(|_| DieselError::RollbackTransaction)?;
        }
        Ok(())
    })?;
    Ok(())
}

/// Insert variables into the `variables` table.
fn insert_variables(
    items: &[VariablesItem],
    conn: &mut PgConnection,
    api_path_id: i32,
    unique_key_constraint: &str,
) -> Result<(), InsertError> {
    use crate::schema::api_paths_variables_association::dsl::*;
    use crate::schema::variables::dsl::variables;

    let variable_ids: Vec<ApiPathsVariablesAssociation> = diesel::insert_into(variables)
        .values(items)
        .on_conflict(on_constraint(unique_key_constraint))
        // UPDATE command is only executed in order to return the `id` column. No value
        // needs to be updated. In other words, `.do_nothing()` only doesn't work because
        // it's not compatible with a RETURNING clause.
        .do_update()
        .set(crate::schema::variables::dsl::name.eq(sql("EXCLUDED.name")))
        .returning(schema::variables::dsl::id)
        .load_iter::<i32, DefaultLoadingMode>(conn)?
        .map(|variable_id| ApiPathsVariablesAssociation {
            // Use a dummy value; otherwise the code won't compile. The postgres database
            // will ignore the dummy and assign its own.
            id: 0,
            api_paths_id: api_path_id,
            variables_id: variable_id.unwrap(),
        })
        .collect();

    diesel::insert_into(api_paths_variables_association)
        .values(&variable_ids)
        .on_conflict_do_nothing()
        .execute(conn)?;
    Ok(())
}

/// Insert geography variables into the `geography` table.
fn insert_geographies(
    items: &[GeographyItem],
    conn: &mut PgConnection,
    api_path_id: i32,
) -> Result<(), InsertError> {
    use crate::schema::api_paths_geography_association::dsl::*;
    use crate::schema::geography::dsl as geography_dsl;

    // Delete geography variables associated with the API that already exist.
    // This ensures against duplicates and is more straightforward than implementing
    // a trigger function in SQL to delete the old geography variables. ON DELETE CASCADE
    // is complicated by the fact that `geography` is the parent to
    // `api_paths_geography_association`.
    let geography_ids_to_delete: Vec<i32> = api_paths_geography_association
        .filter(api_paths_id.eq(api_path_id))
        .select(geography_id)
        .load(conn)?;
    diesel::delete(api_paths_geography_association)
        .filter(api_paths_id.eq(api_path_id))
        .execute(conn)?;
    if !geography_ids_to_delete.is_empty() {
        diesel::delete(geography_dsl::geography)
            .filter(geography_dsl::id.eq_any(geography_ids_to_delete))
            .execute(conn)?;
    }

    let geography_ids: Vec<ApiPathsGeographyAssociation> =
        diesel::insert_into(geography_dsl::geography)
            .values(items)
            .returning(geography_dsl::id)
            .load_iter::<i32, DefaultLoadingMode>(conn)?
            .map(|geo_id| ApiPathsGeographyAssociation {
                // Use a dummy value; otherwise the code won't compile. The postgres database
                // will ignore the dummy and assign its own.
                id: 0,
                api_paths_id: api_path_id,
                geography_id: geo_id.unwrap(),
            })
            .collect();

    diesel::insert_into(api_paths_geography_association)
        .values(&geography_ids)
        .execute(conn)?;
    Ok(())
}
