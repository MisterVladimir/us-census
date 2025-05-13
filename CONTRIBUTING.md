Welcome to the developer docs for `us-census`!
====

# Running Tests

`cargo test`

# Commit code

## pre-commit

This project uses [pre-commit](https://pre-commit.com/) to run linters and formatters before committing code.
To set it up, first create a local Python environment and then install `pre-commit`.
Note the hooks run `rustfmt` which is installed with Rust by default.

```bash
python3 -m venv .venv
. .venv/bin/activate
pip install pre-commit
pre-commit install
```

To run the pre-commit hooks manually, run `pre-commit run --all-files`.
