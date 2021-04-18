FROM rust:1.51 AS builder

COPY . /build
WORKDIR /build
RUN make build

FROM scratch

COPY --from=/build/target/release/ip-update /usr/local/bin/

ENTRYPOINT ["ip-update"]
