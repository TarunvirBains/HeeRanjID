FROM rust:1.94-slim-bookworm

RUN apt-get update && apt-get install -y --no-install-recommends \
        git curl make python3 python3-pip python3-venv nodejs npm \
    && pip install maturin --break-system-packages \
    && npm install -g @napi-rs/cli \
    && rustup component add rustfmt clippy \
    && DENY_VERSION=$(curl -sL https://api.github.com/repos/EmbarkStudios/cargo-deny/releases/latest \
       | grep -o '"tag_name": *"[^"]*"' | head -1 | cut -d'"' -f4) \
    && curl -sL "https://github.com/EmbarkStudios/cargo-deny/releases/download/${DENY_VERSION}/cargo-deny-${DENY_VERSION}-x86_64-unknown-linux-musl.tar.gz" \
       | tar xz -C /usr/local/bin --strip-components=1 --wildcards '*/cargo-deny' \
    && apt-get clean && rm -rf /var/lib/apt/lists/*
