use crate::schema::api_paths;
use crate::schema::api_paths_geography_association;
use crate::schema::api_paths_variables_association;
use diesel::prelude::*;
use serde::Deserialize;
use std::borrow::Cow;

/// The metadata of each US Census API endpoint, as provided in each element of
/// https://api.census.gov/data.json
#[derive(Deserialize, Queryable, Identifiable, Selectable, Debug, PartialEq, Insertable)]
#[diesel(table_name = api_paths)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct ApiPaths<'a> {
    #[serde(skip, default)]
    #[diesel(skip_insertion)]
    pub id: i32,
    pub c_vintage: Option<i32>,
    pub c_dataset: Vec<Option<Cow<'a, str>>>,
    #[serde(rename = "c_geographyLink")]
    pub c_geography_link: Cow<'a, str>,
    #[serde(rename = "c_variablesLink")]
    pub c_variables_link: Cow<'a, str>,
    pub title: Cow<'a, str>,
    pub description: Cow<'a, str>,
}

/// This is the top-level item at https://api.census.gov/data.json.
#[derive(Deserialize, Debug)]
pub struct UsCensusApisResponse<'a> {
    pub dataset: Vec<ApiPaths<'a>>,
}

/// Association table that enables a many-to-many relationship between
/// the `api_paths` and `variables` tables.
#[derive(Deserialize, Queryable, Identifiable, Selectable, Debug, PartialEq, Insertable)]
#[diesel(table_name = api_paths_variables_association)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct ApiPathsVariablesAssociation {
    #[diesel(skip_insertion)]
    pub id: i32,
    pub api_paths_id: i32,
    pub variables_id: i32,
}
/// Association table that enables a many-to-many relationship between
/// the `api_paths` and `geography` tables.
#[derive(Deserialize, Queryable, Identifiable, Selectable, Debug, PartialEq, Insertable)]
#[diesel(table_name = api_paths_geography_association)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct ApiPathsGeographyAssociation {
    #[diesel(skip_insertion)]
    pub id: i32,
    pub api_paths_id: i32,
    pub geography_id: i32,
}
