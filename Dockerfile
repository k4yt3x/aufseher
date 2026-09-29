FROM rust:alpine AS builder
COPY . /app
WORKDIR /app
RUN apk add --no-cache musl-dev \
    && cargo build --release --locked --target x86_64-unknown-linux-musl

FROM scratch AS binary
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/aufseher /aufseher

FROM gcr.io/distroless/static:nonroot
LABEL maintainer="K4YT3X <i@k4yt3x.com>" \
      org.opencontainers.image.source="https://github.com/k4yt3x/aufseher" \
      org.opencontainers.image.description="Telegramgruppenzutrittsverweigerungssystem"
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/aufseher \
                    /usr/local/bin/aufseher
USER nonroot:nonroot
ENTRYPOINT ["/usr/local/bin/aufseher"]
