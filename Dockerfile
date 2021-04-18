FROM rust:1.51 AS builder

COPY . /build
WORKDIR /build
RUN make build

FROM debian:stretch-slim

COPY --from=builder /build/target/release/ip-update /usr/local/bin/
RUN ls -l /usr/local/bin

ENTRYPOINT ["/usr/local/bin/ip-update"]
