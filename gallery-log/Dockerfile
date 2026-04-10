FROM rust:latest AS builder

WORKDIR /app

# Copy everything
COPY . .

# Build the binaries
RUN cargo build --release

# Make scripts executable
RUN chmod +x demo.sh reset.sh

# IMPORTANT: Keep the target directory structure for tests
# The tests look for binaries at ./target/release/ relative to CARGO_MANIFEST_DIR

FROM debian:bookworm-slim AS runtime

WORKDIR /app

# Install bash
RUN apt-get update && apt-get install -y \
    bash \
    && rm -rf /var/lib/apt/lists/*

# Copy binaries
COPY --from=builder /app/target/release/logappend ./logappend
COPY --from=builder /app/target/release/logread ./logread

# Copy scripts
COPY --from=builder /app/demo.sh ./demo.sh
COPY --from=builder /app/reset.sh ./reset.sh

# Make scripts executable
RUN chmod +x demo.sh reset.sh

# Create logs directory
RUN mkdir -p /app/logs

ENTRYPOINT []
CMD ["/bin/bash"]