# Stage 1: Build the application with BuildKit cache mounts
FROM rust:slim AS builder

WORKDIR /usr/src/pupoxide

# Install build dependencies
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

# Copy sources from read-only bind mount, then compile using writable cache mounts
RUN --mount=type=bind,source=.,target=/tmp/src \
    --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/src/pupoxide/target \
    cp -R /tmp/src/. . && \
    cargo build --release && \
    cp target/release/pupoxide /usr/local/bin/pupoxide

# Stage 2: Target environment for testing
FROM ubuntu:24.04

# Avoid prompts from apt
ENV DEBIAN_FRONTEND=noninteractive

# Install target system dependencies for testing apt provider
RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    git \
    sudo \
    && rm -rf /var/lib/apt/lists/*

# Copy the compiled binary from the builder stage
COPY --from=builder /usr/local/bin/pupoxide /usr/local/bin/pupoxide

# Create configuration and environment structures for demonstration
WORKDIR /app
COPY examples/ /app/examples/

# Create a simple demonstration entrypoint script
RUN echo '#!/bin/bash\n\
    echo "============================================="\n\
    echo "      Welcome to Pupoxide PoC Environment     "\n\
    echo "============================================="\n\
    echo "Pupoxide version: $(pupoxide --version || echo "unknown")"\n\
    echo "Current OS Family: Ubuntu (apt/dpkg)"\n\
    echo ""\n\
    echo "You can test applying a manifest locally by running:"\n\
    echo "  pupoxide run --file /app/examples/environments/production/manifests/site.rhai --show-unchanged"\n\
    echo ""\n\
    echo "Launching bash session..."\n\
    exec bash' > /app/entrypoint.sh && chmod +x /app/entrypoint.sh

ENTRYPOINT ["/app/entrypoint.sh"]
