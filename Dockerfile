FROM node:22-slim

# 1. Install basic dependencies AND Homebrew prerequisites
RUN apt-get update && apt-get install -y \
    git \
    curl \
    build-essential \
    procps \
    file \
    sudo \
    && rm -rf /var/lib/apt/lists/*

# Install Rust and Cargo using rustup
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

# Install the Kilo Code CLI
RUN npm i -g @kilocode/cli@0.26.0

WORKDIR /home/

# Copy automation directory
COPY automation /home/automation

# Copy Kilo Code Config to /root/.kilocode (root user's home directory)
COPY .kilocode /root/.kilocode

# Default command - accepts LOOP_TYPE environment variable
# Use: docker run -e LOOP_TYPE=development ... (or bugfixer)
CMD ["sh", "-c", "node /home/automation/run.js $LOOP_TYPE"]