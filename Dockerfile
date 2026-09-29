FROM rust:1.85-bookworm AS build
WORKDIR /src
COPY . .
RUN cargo build --release --bin deadbolt

FROM debian:bookworm-slim
RUN useradd --system --create-home --shell /usr/sbin/nologin deadbolt
COPY --from=build /src/target/release/deadbolt /usr/local/bin/deadbolt
USER deadbolt
EXPOSE 9782
ENTRYPOINT ["/usr/local/bin/deadbolt", "serve", "--bind", "127.0.0.1:9782"]
