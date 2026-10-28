# Stage 1: Build Rust
FROM rust:1.88 AS rust-builder
WORKDIR /app

# Build dependencies first (including the vendored rs-auth crates) so they are cached
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN mkdir src && echo "fn main() {}" > src/main.rs && cargo build --release && rm -f src/main.rs

COPY . .

# Touch the sources so cargo rebuilds the app instead of reusing the placeholder binary
RUN touch src/main.rs src/lib.rs && cargo build --release

# Stage 2: Build Tailwind CSS
FROM node AS node-builder
WORKDIR /app

COPY package*.json ./
RUN npm ci

COPY . .
RUN npm run build:css

# Stage 3: Final image
FROM debian:bookworm-slim AS final
WORKDIR /app

RUN useradd -m appuser

COPY --from=rust-builder /app/target/release/exchange-ops ./backend

COPY --from=node-builder /app/static/tailwind.css ./static/tailwind.css
COPY --from=node-builder /app/static/htmx.min.js ./static/htmx.min.js

RUN apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates libmariadb3 \
  && rm -rf /var/lib/apt/lists/*

USER appuser

EXPOSE 4001

CMD ["./backend"]
