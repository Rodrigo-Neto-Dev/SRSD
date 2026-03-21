FROM rust:latest AS builder

WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim

WORKDIR /app
COPY --from=builder /app/target/release/logappend ./logappend
COPY --from=builder /app/target/release/logread ./logread

# Create the logs directory inside the container.
# When running via docker-compose this gets replaced by the bind mount.
RUN mkdir -p /app/logs

ENTRYPOINT ["/bin/bash"]