# Publishing

This repository publishes multiple artifacts from one workspace:

- Rust crates to `crates.io`
- Python packages to PyPI

## Before You Publish

1. Initialize submodules.

```bash
git submodule update --init --recursive
```

2. Verify the release branch is clean and all versions are intentional.
3. Run the relevant dry runs and package builds.

## Rust Release Checklist

Run these before publishing:

```bash
cargo fmt --all --check
cargo clippy --workspace --exclude heeranjid-python --exclude heeranjid-node -- -D warnings
cargo test -p heeranjid --lib
cargo publish -p heeranjid --dry-run
cargo publish -p heeranjid-sqlx --dry-run
cargo publish -p heeranjid-ffi --dry-run
```

If you are validating the database-backed crate, also run:

```bash
DATABASE_URL=postgres://postgres:postgres@localhost:5432/heeranjid cargo test -p heeranjid-sqlx --test postgres
DATABASE_URL=postgres://postgres:postgres@localhost:5432/heeranjid cargo test -p heeranjid-sqlx --test concurrency
```

### Publish Order

Publish Rust crates in dependency order:

```bash
cargo publish -p heeranjid
cargo publish -p heeranjid-sqlx
cargo publish -p heeranjid-ffi
```

Wait for `heeranjid` to become available on `crates.io` before publishing the
dependent crates.

## Python Release Checklist

The Python wheel build expects the `sql/` submodule to be present.

Core package:

```bash
python -m venv .venv
. .venv/bin/activate
python -m pip install --upgrade pip maturin build twine
cd bindings/python
make build
twine check ../../target/wheels/*
```

Django package:

```bash
python -m venv .venv
. .venv/bin/activate
python -m pip install --upgrade pip build twine
cd bindings/python/django
python -m build
twine check dist/*
```

Upload once the artifacts look correct:

```bash
twine upload ../../target/wheels/*.whl
twine upload dist/*
```

## GitHub Actions Publishing

Required secrets:

- `CARGO_REGISTRY_TOKEN`
- `PYPI_TOKEN`

Workflow path:

- Actions
- `Publish`
- choose the package
- run the workflow

## Versioning

Update versions in:

- [`Cargo.toml`](../Cargo.toml)
- [`bindings/python/pyproject.toml`](../bindings/python/pyproject.toml)
- [`bindings/python/django/pyproject.toml`](../bindings/python/django/pyproject.toml)
- [`bindings/typescript/package.json`](../bindings/typescript/package.json) if you are cutting a matching JS release

## Notes

- A local checkout without submodules does not reflect CI behavior.
- The checked-in CI workflow uses `submodules: recursive`.
- `cargo publish --dry-run` is the source of truth for `crates.io` readiness.
