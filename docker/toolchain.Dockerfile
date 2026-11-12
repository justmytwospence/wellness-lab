FROM rust:1.99.1-bookworm
ARG PLAYWRIGHT_VERSION=1.65.0
RUN rustup target add wasm32-unknown-unknown && rustup component add rustfmt clippy
RUN curl -fsSL https://deb.nodesource.com/setup_24.x | bash - && apt-get install -y nodejs python3-pip python3-venv pkg-config \
    && rm -rf /var/lib/apt/lists/*
RUN cargo install wasm-pack --locked --version 0.13.1 && rm -rf /usr/local/cargo/registry
RUN pip install --break-system-packages maturin==1.9.6
COPY --from=ghcr.io/astral-sh/uv:latest /uv /usr/local/bin/uv
ENV PLAYWRIGHT_BROWSERS_PATH=/ms-playwright
RUN npx -y playwright@${PLAYWRIGHT_VERSION} install --with-deps chromium && chmod -R a+rX /ms-playwright
ENV RUSTUP_HOME=/usr/local/rustup
WORKDIR /w
