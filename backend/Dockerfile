# ponytail: no dependency-layer caching, so every image build compiles all crates (~5 min in CI).
# Add cargo-chef if build time starts to hurt.
FROM rust:1-slim-bookworm AS build
# utoipa-swagger-ui's build script downloads the Swagger UI bundle with curl.
RUN apt-get update && apt-get install -y --no-install-recommends curl ca-certificates \
  && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY . .
RUN cargo build --release --locked

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl \
  && rm -rf /var/lib/apt/lists/*
COPY --from=build /app/target/release/expense-backend /usr/local/bin/expense-backend
ENV BIND_ADDR=0.0.0.0:8080
EXPOSE 8080
USER nobody
CMD ["expense-backend"]
