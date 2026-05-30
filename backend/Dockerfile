FROM rust:1.75-slim as builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./backend/src
RUN apt-get update && apt-get install -y \
    libssl-dev pkg-config \
    && rm -rf /var/lib/apt/lists/*
RUN cargo build --release && \
    cp target/release/tlg-rag-bot /app/

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/tlg-rag-bot /usr/local/bin/
WORKDIR /app
COPY .env.example .env
EXPOSE 4000
CMD ["tlg-rag-bot"]