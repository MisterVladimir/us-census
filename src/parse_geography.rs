use crate::schema::geography;
use chrono::NaiveDate;
use diesel::prelude::*;
use serde::{de, Deserialize, Deserializer};
use std::borrow::Cow;
use std::fmt;

#[derive(Deserialize, Insertable, Queryable, Selectable, Identifiable, Debug, PartialEq)]
#[diesel(table_name = geography)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct GeographyItem<'a> {
    // Primary key in the database. This field is not in geography.json.
    #[serde(skip, default)]
    #[diesel(skip_insertion)]
    pub id: i32,
    #[serde(borrow)]
    pub name: Cow<'a, str>,
    #[serde(borrow, default, rename = "geoLevelDisplay")]
    pub geo_level_display: Option<&'a str>,
    #[serde(default, rename = "referenceDate", deserialize_with = "parse_date")]
    reference_date: Option<NaiveDate>,
    #[serde(borrow, default)]
    requires: Option<Vec<&'a str>>,
    #[serde(borrow, default, deserialize_with = "parse_wildcard")]
    wildcard: Option<Vec<&'a str>>,
    #[serde(default, deserialize_with = "parse_limit")]
    limit: Option<i32>,
    #[serde(borrow, default, rename = "geoLevelId")]
    geo_level_id: Option<&'a str>,
    #[serde(borrow, default, rename = "optionalWithWCFor")]
    optional_with_wildcard_for: Option<&'a str>,
}

#[derive(PartialEq, Deserialize, Debug)]
pub struct GeographyCollection<'a> {
    #[serde(borrow, default)]
    pub fips: Vec<GeographyItem<'a>>,
}

/// Deserialize a date string in the format "YYYY-MM-DD" or just "YYYY".
fn parse_date<'de, D>(deserializer: D) -> Result<Option<NaiveDate>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt_str = Option::<String>::deserialize(deserializer)?;

    match opt_str {
        None => Ok(None),
        Some(s) if s.len() == 4 && s.chars().all(|c| c.is_ascii_digit()) => {
            // Handle year-only format
            let year = s.parse::<i32>().map_err(de::Error::custom)?;
            Ok(NaiveDate::from_ymd_opt(year, 1, 1))
        }
        Some(s) => {
            // Handle normal date format
            NaiveDate::parse_from_str(&s, "%Y-%m-%d")
                .map(Some)
                .map_err(de::Error::custom)
        }
    }
}

struct WildcardVisitor;

impl<'de> de::Visitor<'de> for WildcardVisitor {
    type Value = Option<Vec<&'de str>>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("an array of strings or a boolean")
    }

    fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        if v {
            Err(de::Error::custom(
                "Boolean value `true` is not allowed for `wildcard`",
            ))
        } else {
            Ok(Some(Vec::new())) // Convert `false` to an empty array
        }
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: de::SeqAccess<'de>,
    {
        let mut vec = Vec::new();
        while let Some(value) = seq.next_element()? {
            vec.push(value);
        }
        Ok(Some(vec))
    }
}

fn parse_wildcard<'de, D>(deserializer: D) -> Result<Option<Vec<&'de str>>, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_any(WildcardVisitor)
}

struct LimitVisitor;

impl<'de> de::Visitor<'de> for LimitVisitor {
    type Value = Option<i32>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a string or integer")
    }

    /// If the 'limit' field is already an integer, just return it.
    fn visit_i32<E>(self, v: i32) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(Some(v))
    }

    /// Convert a string to an integer, stripping any quotation marks.
    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        let cleaned_str = v.trim_matches('"');
        let limit = cleaned_str
            .parse::<i32>()
            .map_err(|_| E::custom(format!("invalid value for 'limit' field: {}", v)))?;

        Ok(Some(limit))
    }
}

fn parse_limit<'de, D>(deserializer: D) -> Result<Option<i32>, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_any(LimitVisitor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use regex::Regex;
    use rstest::{fixture, rstest};
    use serde_json::{from_str, json, to_string, Map, Value};

    #[rstest]
    // Test that the JSON parsing works correctly for a valid input
    // whose only fields are `name` and `referenceDate`.
    fn test_parse_valid_date() {
        let object_under_test = r#"
    {
      "fips": [
        {
          "name": "us",
          "referenceDate": "2010-01-01"
        }
      ]
    }"#;
        let result: GeographyCollection = from_str(&object_under_test).expect("Error parsing JSON");
        let expected = GeographyCollection {
            fips: vec![GeographyItem {
                id: 0,
                name: Cow::from("us"),
                geo_level_display: None,
                reference_date: NaiveDate::from_ymd_opt(2010, 1, 1),
                geo_level_id: None,
                requires: None,
                wildcard: None,
                limit: None,
                optional_with_wildcard_for: None,
            }],
        };
        assert_eq!(result, expected);
    }

    /// Test `referenceDate` field that only contains a year.
    #[rstest]
    fn test_date_year_only() {
        // Arrange
        let object_under_test = r#"
        {
          "fips": [
            {
              "name": "us",
              "referenceDate": "2010"
            }
          ]
        }"#;
        let expected = GeographyCollection {
            fips: vec![GeographyItem {
                id: 0,
                name: Cow::from("us"),
                geo_level_display: None,
                reference_date: NaiveDate::from_ymd_opt(2010, 1, 1),
                geo_level_id: None,
                requires: None,
                wildcard: None,
                limit: None,
                optional_with_wildcard_for: None,
            }],
        };

        // Act
        let result: GeographyCollection = from_str(&object_under_test).expect("Error parsing JSON");

        // Assert
        assert_eq!(result, expected);
    }

    /// Return a JSON string that follows the happy path when deserialized to a
    /// `GeographyItem`. Field will be substituted to test specific cases.
    #[fixture]
    fn base_value() -> Map<String, Value> {
        let serialized_value = json!({
            "name": "us",
            "geoLevelDisplay": "010",
            "referenceDate": "2010-01-01",
            "wildcard": false,
            "limit": 10,
            "geoLevelId": "010",
            "optionalWithWCFor": "us"
        });

        serialized_value.as_object().unwrap().clone()
    }

    /// String 'limit' values containing quotes are parsed into integers.
    #[rstest]
    fn test_limit_string(mut base_value: Map<String, Value>) {
        // Arrange
        base_value.insert("limit".to_string(), Value::String("51".to_string()));
        let object_under_test = json!({
            "fips": [Value::Object(base_value)]
        });
        let object_under_test_str = to_string(&object_under_test).unwrap();

        // Act
        let result: GeographyCollection =
            from_str(&object_under_test_str).expect("Error parsing JSON");

        // Assert
        assert_eq!(result.fips[0].limit, Some(51));
    }

    /// String 'limit' values containing quotes are parsed into integers.
    #[rstest]
    fn test_limit_quotes(mut base_value: Map<String, Value>) {
        // Arrange
        base_value.insert("limit".to_string(), Value::String("\"51".to_string()));
        let object_under_test = json!({
            "fips": [Value::Object(base_value)]
        });
        let object_under_test_str = to_string(&object_under_test).unwrap();

        // Act
        let result: GeographyCollection =
            from_str(&object_under_test_str).expect("Error parsing JSON");

        // Assert
        assert_eq!(result.fips[0].limit, Some(51));
    }

    /// Test that 'limit' values can be parsed from string
    #[rstest]
    fn test_limit_big_value(mut base_value: Map<String, Value>) {
        // Arrange
        base_value.insert("limit".to_string(), Value::String("65536".to_string()));
        let object_under_test = json!({
            "fips": [Value::Object(base_value)]
        });
        let object_under_test_str = to_string(&object_under_test).unwrap();

        // Act
        let result: GeographyCollection =
            from_str(&object_under_test_str).expect("Error parsing JSON");

        // Assert
        assert_eq!(result.fips[0].limit, Some(65536));
    }

    /// Missing 'fips' field
    #[rstest]
    fn test_missing_fips() {
        let object_under_test = r#"
        {
          "default": [
            {
              "isDefault": "true"
            }
          ]
        }"#;
        let result: GeographyCollection = from_str(&object_under_test).expect("Error parsing JSON");
        assert_eq!(result.fips.len(), 0);
    }

    #[rstest]
    fn test_false_wildcard() {
        let object_under_test = r#"
    {
      "fips": [
        {
          "name": "us",
          "geoLevelDisplay": "010",
          "referenceDate": "2010-01-01",
          "wildcard": false
        }
      ]
    }"#;
        let result: GeographyCollection = from_str(&object_under_test).expect("Error parsing JSON");
        let expected = GeographyCollection {
            fips: vec![GeographyItem {
                id: 0,
                name: Cow::from("us"),
                geo_level_display: Option::from("010"),
                reference_date: NaiveDate::from_ymd_opt(2010, 1, 1),
                geo_level_id: None,
                requires: None,
                wildcard: Option::from(Vec::new()),
                limit: None,
                optional_with_wildcard_for: None,
            }],
        };
        assert_eq!(result, expected);
    }

    /// 'wildcard' field is a bool with value `true`.
    #[rstest]
    fn test_true_wildcard() {
        let invalid_json = r#"
    {
      "fips": [
        {
          "name": "us",
          "geoLevelDisplay": "010",
          "referenceDate": "2010-01-01",
          "wildcard": true
        }
      ]
    }"#;
        let result: Result<GeographyCollection, _> = from_str(&invalid_json);
        assert!(result.is_err());
        if let Err(err) = result {
            let expected_error_message_re = Regex::new(r".*true.*wildcard.*").unwrap();
            assert!(
                expected_error_message_re.is_match(&err.to_string()),
                "Unexpected error message: {}",
                err.to_string()
            );
        }
    }
}
