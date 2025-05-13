-- Create IMMUTABLE versions of various built-in functions in order
-- to use them in a UNIQUE constraint.
CREATE FUNCTION immutable_md5(input TEXT) RETURNS TEXT
AS
    $$
BEGIN
RETURN md5(input);
END;
$$
LANGUAGE plpgsql IMMUTABLE;

CREATE FUNCTION immutable_array_to_string(input TEXT[], delimiter TEXT) RETURNS TEXT
AS
    $$
BEGIN
RETURN array_to_string(input, delimiter);
END;
$$
LANGUAGE plpgsql IMMUTABLE;

CREATE TABLE geography
(
    id                         SERIAL PRIMARY KEY,
    name                       TEXT NOT NULL CHECK (name <> ''),
    geo_level_display          TEXT,
    reference_date             DATE,
    requires                   TEXT[],
    wildcard                   TEXT[] DEFAULT NULL,
    "limit"                    INT,
    geo_level_id               TEXT,
    optional_with_wildcard_for TEXT
);

CREATE TABLE variables
(
    id              SERIAL PRIMARY KEY,
--     Name of the variable in the API
    name            TEXT NOT NULL CHECK (name <> ''),
    label           TEXT[] NOT NULL,
    concept         TEXT,
    required        TEXT,
    predicate_type  TEXT,
    "group"         TEXT[],
    "limit"         SMALLINT,
    predicate_only  BOOLEAN,
    attributes      TEXT[],
--     Create a UNIQUE constraint using hashes because `concept` and `attributes`
--     columns are often very large and violate the PostgreSQL limit of 2712 bytes.
--     Similarly, most of the time, the `group` contains one element but can be
--     extremely large for one or two values per API endpoint. Given the `group` values
--     in the current US census API, as of 2025-05-13, comparing the first element of `group`
--     is sufficient to ensure uniqueness.
    _first_group     TEXT GENERATED ALWAYS AS (COALESCE("group"[0], '')) STORED,
    _concept_hash    TEXT GENERATED ALWAYS AS (immutable_md5(COALESCE(concept, ''))) STORED,
    _attributes_hash TEXT GENERATED ALWAYS AS (
        immutable_md5(immutable_array_to_string(COALESCE(attributes, '{}'), ','))) STORED,
    UNIQUE (name, _attributes_hash, _concept_hash, _first_group)
);

CREATE TABLE api_paths
(
    id               SERIAL PRIMARY KEY,
    c_vintage        INTEGER,
    c_dataset        TEXT[] NOT NULL DEFAULT '{}',
    c_geography_link TEXT NOT NULL,
    c_variables_link TEXT NOT NULL,
    title            TEXT NOT NULL,
    description      TEXT NOT NULL,
    UNIQUE (c_vintage, c_dataset)
);

CREATE TABLE api_paths_variables_association
(
    id           SERIAL PRIMARY KEY,
    api_paths_id INT NOT NULL REFERENCES api_paths (id),
    variables_id INT NOT NULL REFERENCES variables (id),
    UNIQUE (api_paths_id, variables_id)
);

CREATE TABLE api_paths_geography_association
(
    id           SERIAL PRIMARY KEY,
    api_paths_id INT NOT NULL REFERENCES api_paths (id),
    geography_id INT NOT NULL REFERENCES geography (id),
    UNIQUE (api_paths_id, geography_id)
);