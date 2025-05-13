Welcome to the docs for `us-census`!
====

# Overview

`us-census` provides an interface to the US Census API, caching the specifications for each
endpoint and query results in a postgres database.
This is very much a work in progress and the main goal is for me to have fun and learn Rust.
If you're looking for an ergonomic interface to the US Census API, you might want to check out
https://github.com/censusdis/censusdis.

# Build

`cargo build --release`

# Run a sample script

`src/main.rs` downloads the US Census API top-level metadata and inserts a few endpoints'
variables and geography parameters into a postgres database.

## Pre-requisites

- [PostgreSQL](https://www.postgresql.org/download/)
- PostgreSQL development headers. For Ubuntu, that's `sudo apt-get install libpq-dev`.
- [rustup](https://rustup.rs/) and the latest stable version of Rust.
- [diesel-cli](https://diesel.rs/guides/getting-started#installing-diesel-cli)
- docker and [docker-compose](https://docs.docker.com/compose/install/)

To run the script, first create a PostgreSQL database in a docker container: `docker compose up -d`.
Then, run the migrations in `migrations/` using diesel-cli, `diesel migration run`.
To run the script: `cargo run --package us_census --bin us_census --release`
Note that this will cache the API metadata in a local data/ directory.

See [CONTRIBUTING.md](CONTRIBUTING.md) for developer instructions.
