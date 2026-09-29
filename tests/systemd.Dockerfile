FROM rust:1.85-bookworm AS build
WORKDIR /src
COPY . .
RUN cargo build --release --locked --bin deadbolt

FROM ubuntu:24.04
RUN apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends systemd ca-certificates curl python3 && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/target/release/deadbolt /usr/local/bin/deadbolt
COPY dist/deadbolt.service /etc/systemd/system/deadbolt.service
RUN useradd --system --user-group --home-dir /var/lib/deadbolt --shell /usr/sbin/nologin deadbolt && install -d -m 0700 /etc/deadbolt && systemctl enable deadbolt
STOPSIGNAL SIGRTMIN+3
CMD ["/lib/systemd/systemd"]
