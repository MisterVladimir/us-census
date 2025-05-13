use diesel::dsl::sql;
use diesel::prelude::*;
use diesel::sql_types::Bool;
use reqwest::Client;
use std::path::Path;
use std::str::FromStr;
use url::Url;
use us_census::constraints::get_unique_constraints;
use us_census::fetch_api_metadata::CachedClient;
use us_census::models::{ApiPaths, UsCensusApisResponse};
use us_census::{establish_database_connection, insert_variables_and_geography_for_api_path};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use us_census::schema::api_paths::dsl::api_paths as dsl_api_paths;

    let conn = &mut establish_database_connection(None, None)?;

    let web_client = Client::new();
    let base_cache_dir = Path::new(".").canonicalize()?;

    let client_with_cache = CachedClient::new(base_cache_dir.to_path_buf(), &web_client);

    // Assume that if there's one API path, we've already added all of them to the database.
    let one_api_path = dsl_api_paths
        .limit(1)
        .select(ApiPaths::as_select())
        .load(conn)?;
    if one_api_path.len() == 0 {
        let api_paths_url = Url::from_str("https://api.census.gov/data.json")?;
        let response_text = client_with_cache.fetch(&api_paths_url).await?;
        let us_census_apis: UsCensusApisResponse = serde_json::from_str(&response_text)?;
        diesel::insert_into(dsl_api_paths)
            .values(&us_census_apis.dataset)
            .execute(conn)?;
    }

    let variables_unique_key_constraints = get_unique_constraints(conn, "variables")?;
    if variables_unique_key_constraints.len() != 1 {
        return Err(Box::from(format!(
            "Expected exactly one unique key constraint for the `variables` table, found {}",
            variables_unique_key_constraints.len()
        )));
    }

    // Insert ACS survey variables and geographies into the database.
    let variables_url_regex = "http://api.census.gov/data/\\d\\d\\d\\d/acs/acs\\d/variables.json";
    let to_insert = dsl_api_paths
        .filter(sql::<Bool>(
            format!("c_variables_link ~ '{}'", variables_url_regex).as_str(),
        ))
        .load::<ApiPaths>(conn)?;
    for metadata in to_insert {
        insert_variables_and_geography_for_api_path(
            conn,
            &client_with_cache,
            &metadata,
            &variables_unique_key_constraints[0],
        )
        .await
        .expect(format!("Error inserting variables: {}", metadata.c_variables_link).as_str());
    }
    Ok(())
}
