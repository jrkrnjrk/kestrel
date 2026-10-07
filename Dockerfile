FROM rust:1.85-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock* ./
COPY src ./src
COPY templates ./templates
COPY static ./static
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=build /src/target/release/kestrel /app/kestrel
COPY --from=build /src/static /app/static
ENV STATIC_DIR=/app/static
EXPOSE 8080
CMD ["/app/kestrel"]
