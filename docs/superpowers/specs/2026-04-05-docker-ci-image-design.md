# Custom Docker CI Image Design

## Goal

A Dockerfile published to GHCR that bakes in all build tools, eliminating ~60-90s of install steps from the Rust CI job. Auto-built by a CI workflow when the Dockerfile changes.

## Design

### Dockerfile

Located at `docker/ci/Dockerfile`:

```dockerfile
FROM rust:1.94-slim-bookworm

RUN apt-get update && apt-get install -y --no-install-recommends \
    git curl make \
    python3 python3-pip python3-venv \
    nodejs npm \
    && pip install maturin --break-system-packages \
    && npm install -g @napi-rs/cli \
    && rustup component add rustfmt clippy \
    && DENY_VERSION=$(curl -sL https://api.github.com/repos/EmbarkStudios/cargo-deny/releases/latest \
       | grep -o '"tag_name": *"[^"]*"' | head -1 | cut -d'"' -f4) \
    && curl -sL "https://github.com/EmbarkStudios/cargo-deny/releases/download/${DENY_VERSION}/cargo-deny-${DENY_VERSION}-x86_64-unknown-linux-musl.tar.gz" \
       | tar xz -C /usr/local/bin --strip-components=1 --wildcards '*/cargo-deny' \
    && apt-get clean && rm -rf /var/lib/apt/lists/*
```

Published to: `ghcr.io/tarunvirbains/heeranjid-ci:latest`

### CI Workflow: `docker/ci.yml`

```yaml
name: Build CI Image

on:
  push:
    branches: [main]
    paths: ['docker/ci/**']
  workflow_dispatch:

jobs:
  build:
    runs-on: ubuntu-latest
    permissions:
      packages: write
    steps:
      - uses: actions/checkout@v4
      - uses: docker/login-action@v3
        with:
          registry: ghcr.io
          username: ${{ github.actor }}
          password: ${{ secrets.GITHUB_TOKEN }}
      - uses: docker/build-push-action@v6
        with:
          context: docker/ci
          push: true
          tags: ghcr.io/tarunvirbains/heeranjid-ci:latest
```

### Main CI Workflow Change

The Rust job changes from:

```yaml
container:
  image: rust:1.94-slim-bookworm
```

to:

```yaml
container:
  image: ghcr.io/tarunvirbains/heeranjid-ci:latest
```

And removes the "Install system dependencies", "Install Rust tools", and "Install build tools" steps — they're baked in.

### Future Images

As more framework integrations are added, additional Dockerfiles can be added under `docker/`:

```
docker/
  ci/
    Dockerfile          # Rust build image (current)
  python/
    Dockerfile          # Python test image with ODBC driver (future)
  dotnet/
    Dockerfile          # .NET test image (future)
```

Each gets its own workflow trigger on path changes.

## What's NOT in scope

- Multi-arch builds (linux/arm64) — YAGNI until someone needs ARM CI
- Versioned tags (`:v1`, `:v2`) — use `:latest` until there's a reason to pin
- Python/TypeScript/.NET test images — they use stock images for now
