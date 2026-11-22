FROM rust:slim-bookworm AS builder
WORKDIR /app
COPY Cargo.toml ./
COPY src ./src
RUN cargo build --release --bin security-monitoring-ai-service

FROM python:3.11-slim-bookworm AS onnx-runtime
RUN pip install --no-cache-dir --no-deps --only-binary=:all: --target /opt/runtime onnxruntime==1.24.4

FROM debian:bookworm-slim
WORKDIR /app
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates ffmpeg libgomp1 && rm -rf /var/lib/apt/lists/*
COPY --from=onnx-runtime /opt/runtime/onnxruntime/capi/libonnxruntime*.so* /opt/onnxruntime/
ENV ORT_DYLIB_PATH=/opt/onnxruntime/libonnxruntime.so.1.24.4
COPY --from=builder /app/target/release/security-monitoring-ai-service /app/security-monitoring-ai-service
EXPOSE 8001
CMD ["/app/security-monitoring-ai-service"]
