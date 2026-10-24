# Build stage
FROM rust:1-bookworm AS builder
RUN apt-get update && apt-get install -y --no-install-recommends libsqlite3-dev pkg-config && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY Cargo.toml ./
COPY src ./src
COPY templates ./templates
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libsqlite3-0 \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --home-dir /app app
WORKDIR /app
COPY --from=builder /app/target/release/it-desk /app/it-desk
COPY static ./static
RUN mkdir -p /data && chown -R app:app /app /data
USER app
ENV PORT=3000
ENV DB_PATH=/data/tickets.db
EXPOSE 3000
CMD ["/app/it-desk"]
