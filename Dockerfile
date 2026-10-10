FROM rust:1.85-bookworm AS build
WORKDIR /src
COPY . .
RUN cargo build --locked --release --bin deadbolt

FROM debian:bookworm-slim
RUN useradd --uid 10001 --create-home --shell /usr/sbin/nologin deadbolt \
    && mkdir -p /home/deadbolt/.deadbolt \
    && chmod 0700 /home/deadbolt/.deadbolt \
    && chown deadbolt:deadbolt /home/deadbolt/.deadbolt
COPY --from=build /src/target/release/deadbolt /usr/local/bin/deadbolt
USER deadbolt
ENTRYPOINT ["/usr/local/bin/deadbolt"]
CMD ["serve"]
