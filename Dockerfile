FROM rust:1.51 AS builder

COPY . /build
WORKDIR /build
RUN make build

FROM scratch

COPY --from=builder /build/target/release/ip-update /usr/local/bin/

ENTRYPOINT ["/usr/local/bin/ip-update"]
