FROM rust:alpine as builder
COPY . /app
WORKDIR /app
RUN apk add --no-cache --virtual .build-deps \
        make \
        musl-dev \
        openssl-dev \
        openssl-libs-static \
        perl \
        pkgconfig \
    && cargo build --release --target x86_64-unknown-linux-musl

FROM gcr.io/distroless/static:nonroot
LABEL maintainer="K4YT3X <i@k4yt3x.com>" \
      org.opencontainers.image.source="https://github.com/k4yt3x/aufseher" \
      org.opencontainers.image.description="Telegramgruppenzutrittsverweigerungssystem"
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/aufseher \
                    /usr/local/bin/aufseher
USER nonroot:nonroot
ENTRYPOINT ["/usr/local/bin/aufseher"]
