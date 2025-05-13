// @generated automatically by Diesel CLI.

diesel::table! {
    api_paths (id) {
        id -> Int4,
        c_vintage -> Nullable<Int4>,
        c_dataset -> Array<Nullable<Text>>,
        c_geography_link -> Text,
        c_variables_link -> Text,
        title -> Text,
        description -> Text,
    }
}

diesel::table! {
    api_paths_geography_association (id) {
        id -> Int4,
        api_paths_id -> Int4,
        geography_id -> Int4,
    }
}

diesel::table! {
    api_paths_variables_association (id) {
        id -> Int4,
        api_paths_id -> Int4,
        variables_id -> Int4,
    }
}

diesel::table! {
    geography (id) {
        id -> Int4,
        name -> Text,
        geo_level_display -> Nullable<Text>,
        reference_date -> Nullable<Date>,
        requires -> Nullable<Array<Nullable<Text>>>,
        wildcard -> Nullable<Array<Nullable<Text>>>,
        limit -> Nullable<Int4>,
        geo_level_id -> Nullable<Text>,
        optional_with_wildcard_for -> Nullable<Text>,
    }
}

diesel::table! {
    variables (id) {
        id -> Int4,
        name -> Text,
        label -> Array<Nullable<Text>>,
        concept -> Nullable<Text>,
        required -> Nullable<Text>,
        predicate_type -> Nullable<Text>,
        group -> Nullable<Array<Nullable<Text>>>,
        limit -> Nullable<Int2>,
        predicate_only -> Nullable<Bool>,
        attributes -> Nullable<Array<Nullable<Text>>>,
        _first_group -> Nullable<Text>,
        _concept_hash -> Nullable<Text>,
        _attributes_hash -> Nullable<Text>,
    }
}

diesel::joinable!(api_paths_geography_association -> api_paths (api_paths_id));
diesel::joinable!(api_paths_geography_association -> geography (geography_id));
diesel::joinable!(api_paths_variables_association -> api_paths (api_paths_id));
diesel::joinable!(api_paths_variables_association -> variables (variables_id));

diesel::allow_tables_to_appear_in_same_query!(
    api_paths,
    api_paths_geography_association,
    api_paths_variables_association,
    geography,
    variables,
);
