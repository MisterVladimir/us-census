use crate::schema::variables;

use diesel::prelude::*;
use regex::Regex;
use serde::de::Visitor;
use serde::{de, Deserialize, Deserializer};
use std::borrow::Cow;
use std::fmt;
use std::sync::OnceLock;

/// `VariablesItem` is a single variable in the variables.json of an API endpoint.
/// Functions that parse the variables.json file will return a `Vec<VariablesItem>` and
/// `VariablesItem` is also used directly reading and writing to the postgres database.
#[derive(Deserialize, Insertable, Queryable, Selectable, Identifiable, Debug, PartialEq)]
#[diesel(table_name = variables)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct VariablesItem<'a> {
    /// Primary key in the database. This field is not in variables.json although for the class
    /// to be insertable `id` must be an `i32` and not an `Option<i32>`.
    #[serde(skip, default)]
    #[diesel(skip_insertion)]
    pub id: i32,
    /// The name of the variable. In variables.json, this is the key of each item
    /// in the top-level "variables" map. The remaining fields are the values. `default` is
    /// required for the implementation of `VariablesItemVisitor.visit_map`.
    #[serde(borrow, default)]
    pub name: Cow<'a, str>,
    /// `label` field must be a `Vec<Cow<'a, str>>` to parse backslashes. Due to how
    /// serde_json parses, backslashes must be owned.
    #[serde(borrow, deserialize_with = "parse_label")]
    pub label: Vec<Cow<'a, str>>,
    // `concept` must be owned to parse escaped quote characters.
    #[serde(borrow)]
    pub concept: Option<Cow<'a, str>>,
    #[serde(borrow)]
    pub required: Option<&'a str>,
    #[serde(borrow, rename = "predicateType")]
    pub predicate_type: Option<&'a str>,
    #[serde(borrow, deserialize_with = "parse_comma_separated_string")]
    pub group: Option<Vec<Cow<'a, str>>>,
    pub limit: Option<i16>,
    #[serde(rename = "predicateOnly")]
    pub predicate_only: Option<bool>,
    #[serde(borrow, default, deserialize_with = "parse_comma_separated_string")]
    pub attributes: Option<Vec<Cow<'a, str>>>,
}

/// `VariablesCollection` is a parsed variables.json file of an API endpoint.
///
/// The variables.json contains a top-level "variables" key, whose items describe
/// query parameters of the endpoint. Each item is parsed into a `VariablesItem`
/// using the `deserialize_variables` function.
#[derive(PartialEq, Deserialize, Debug)]
pub struct VariablesCollection<'a> {
    #[serde(borrow, deserialize_with = "deserialize_variables")]
    pub variables: Vec<VariablesItem<'a>>,
}

/// Generic Visitor trait for deserializing a string field in `variables.json` into
/// a list of strings.
trait StringToVecVisitorConfig {
    /// The character to remove from the end of the string.
    const TRIM_CHAR: char;
    /// The description of the expected format, used in error messages.
    const DESCRIPTION: &'static str;

    /// Return the cached regular expression for splitting the string.
    fn get_split_regex() -> &'static Regex;
}

/// The regular expression for splitting the `label` field into a list.
/// This is used in `LabelVisitorConfig`.
static LABEL_REGEX: OnceLock<Regex> = OnceLock::new();

/// Visitor for deserializing the `label` field in `variables.json`.
struct LabelVisitorConfig;
impl StringToVecVisitorConfig for LabelVisitorConfig {
    /// The regular expression for splitting the `label` field.
    const TRIM_CHAR: char = ':';
    const DESCRIPTION: &'static str = "words separated by '!!:`, '!!', or `:`";

    fn get_split_regex() -> &'static Regex {
        // This will never panic since it's validated at compile time (see below).
        LABEL_REGEX.get_or_init(|| {
            Regex::new(r":?!!").expect("Invalid regular expression -- this is a bug.")
        })
    }
}

/// The regular expression for splitting a string by a comma delimiter.
/// This is used in `CommaSplitVisitorConfig`.
static COMMA_REGEX: OnceLock<Regex> = OnceLock::new();
/// Visitor for deserializing a comma-separated field in `variables.json`.
struct CommaSplitVisitorConfig;
impl StringToVecVisitorConfig for CommaSplitVisitorConfig {
    const TRIM_CHAR: char = ' ';
    const DESCRIPTION: &'static str = "comma-separated words";

    fn get_split_regex() -> &'static Regex {
        COMMA_REGEX
            .get_or_init(|| Regex::new(r",").expect("Invalid regular expression -- this is a bug."))
    }
}

/// Generic visitor that uses a `VisitorConfig` to determine how to further parse
/// deserialized JSON into a list. Ultimately the output is used to construct the
/// `VariablesItem` and other structs.
struct StringToVecVisitor<T: StringToVecVisitorConfig>(std::marker::PhantomData<T>);

impl<T: StringToVecVisitorConfig> StringToVecVisitor<T> {
    fn new() -> Self {
        Self(std::marker::PhantomData)
    }
}

impl<'de, T: StringToVecVisitorConfig> Visitor<'de> for StringToVecVisitor<T> {
    type Value = Vec<Cow<'de, str>>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("words separated by '!!:`, '!!', or `:`")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        let re = T::get_split_regex();
        let removed_terminal_colon = v.trim_matches(T::TRIM_CHAR);
        Ok(re
            .split(removed_terminal_colon)
            .map(|s| Cow::Owned(s.to_string()))
            .collect())
    }

    /// Same as `visit_str`, but for borrowed strings.
    fn visit_borrowed_str<E>(self, v: &'de str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        let re = T::get_split_regex();
        let removed_terminal_colon = v.trim_matches(T::TRIM_CHAR);
        Ok(re
            .split(removed_terminal_colon)
            .map(|s| Cow::Borrowed(s))
            .collect())
    }
}

/// Deseralize the `label` field in `variables.json` into a list of strings.
fn parse_label<'de, D>(deserializer: D) -> Result<Vec<Cow<'de, str>>, D::Error>
where
    D: Deserializer<'de>,
{
    let visitor = StringToVecVisitor::<LabelVisitorConfig>::new();
    deserializer.deserialize_str(visitor)
}

fn parse_comma_separated_string<'de, D>(
    deserializer: D,
) -> Result<Option<Vec<Cow<'de, str>>>, D::Error>
where
    D: Deserializer<'de>,
{
    let visitor = StringToVecVisitor::<CommaSplitVisitorConfig>::new();
    let deserialization_result = deserializer.deserialize_str(visitor)?;
    Ok(Some(deserialization_result))
}

struct VariablesItemVisitor;

impl<'de> Visitor<'de> for VariablesItemVisitor {
    type Value = Vec<VariablesItem<'de>>;

    /// Create the error message for the `visit_map` function.
    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a map of variables")
    }

    /// Deserialize the items in variables.json into a list of `VariablesItem`.
    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: de::MapAccess<'de>,
    {
        let mut variables = Vec::new();
        while let Some((key, value)) = map.next_entry::<&'de str, VariablesItem<'de>>()? {
            variables.push(VariablesItem {
                id: value.id,
                name: Cow::from(key),
                label: value.label,
                concept: value.concept,
                required: value.required,
                predicate_type: value.predicate_type,
                group: value.group,
                limit: value.limit,
                predicate_only: value.predicate_only,
                attributes: value.attributes,
            });
        }
        Ok(variables)
    }
}

/// Deserialize the `variables` field in variables.json into a list of `VariablesItem`.
/// See `VariablesItemVisitor.visit_map` for details.
fn deserialize_variables<'de, D>(deserializer: D) -> Result<Vec<VariablesItem<'de>>, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_map(VariablesItemVisitor)
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn test_parse_backslashes() {
        let object_under_test = r#"
    {
      "variables": {
        "a": {
          "label": "foo!!bar!! \"baz\"",
          "predicateType": "int",
          "group": "g1,g2,g3,g4",
          "limit": 0,
          "attributes": "A,B,C"
        },
        "b": {
          "label": "qux!!quux!!corge",
          "predicateType": "int",
          "group": "g2",
          "limit": 0,
          "attributes": "D,E,F"
        }
      }
    }"#;
        let result: VariablesCollection =
            serde_json::from_str(&object_under_test).expect("Error parsing JSON");
        let expected = VariablesCollection {
            variables: vec![
                VariablesItem {
                    id: 0,
                    name: Cow::from("a"),
                    label: vec![Cow::from("foo"), Cow::from("bar"), Cow::from(" \"baz\"")],
                    concept: None,
                    required: None,
                    predicate_type: Option::from("int"),
                    group: Option::from(vec![
                        Cow::from("g1"),
                        Cow::from("g2"),
                        Cow::from("g3"),
                        Cow::from("g4"),
                    ]),
                    limit: Option::from(0),
                    predicate_only: None,
                    attributes: Option::from(vec![Cow::from("A"), Cow::from("B"), Cow::from("C")]),
                },
                VariablesItem {
                    id: 0,
                    name: Cow::from("b"),
                    label: vec![Cow::from("qux"), Cow::from("quux"), Cow::from("corge")],
                    concept: None,
                    required: None,
                    predicate_type: Option::from("int"),
                    group: Option::from(vec![Cow::from("g2")]),
                    limit: Option::from(0),
                    predicate_only: None,
                    attributes: Option::from(vec![Cow::from("D"), Cow::from("E"), Cow::from("F")]),
                },
            ],
        };
        assert_eq!(result, expected);
        // Assert that values are borrowed or owned as expected.
        let a_item = &result.variables[0];
        matches!(a_item.label[0], Cow::Borrowed(_));
        matches!(a_item.label[1], Cow::Borrowed(_));
        matches!(a_item.label[2], Cow::Owned(_)); // backslashes are owned
        let b_item = &result.variables[1];
        matches!(b_item.label[0], Cow::Borrowed(_));
        matches!(b_item.label[1], Cow::Borrowed(_));
        matches!(b_item.label[2], Cow::Borrowed(_));
    }
}
