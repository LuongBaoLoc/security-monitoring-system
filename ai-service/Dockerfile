FROM rust:slim-bookworm AS builder
WORKDIR /app
COPY Cargo.toml ./
COPY src ./src
RUN cargo build --release --bin security-monitoring-ai-service

FROM debian:bookworm-slim
WORKDIR /app
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates ffmpeg && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/security-monitoring-ai-service /app/security-monitoring-ai-service
EXPOSE 8001
CMD ["/app/security-monitoring-ai-service"]
