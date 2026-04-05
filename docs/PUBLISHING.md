# Publishing

## Prerequisites

- `CARGO_REGISTRY_TOKEN` secret set in GitHub repo settings (for crates.io)
- `PYPI_TOKEN` secret set in GitHub repo settings (for PyPI)

## Publishing via GitHub Actions

Go to Actions → "Publish" → Run workflow → Select package → Run.

## Publishing manually

### Rust crates (crates.io)
```bash
cargo publish -p heeranjid
cargo publish -p heeranjid-sqlx
cargo publish -p heeranjid-ffi
```

### Python (PyPI)
```bash
cd bindings/python && make build
twine upload target/wheels/*.whl

cd bindings/python/django
python -m build && twine upload dist/*
```

## Version bumping

Update version in:
- `Cargo.toml` (workspace.package.version) for Rust crates
- `bindings/python/pyproject.toml` for Python
- `bindings/python/django/pyproject.toml` for Django
