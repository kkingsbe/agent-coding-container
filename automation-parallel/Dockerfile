# Use Node.js base image
FROM node:22-slim

# Set working directory
WORKDIR /app

# Add labels for metadata
LABEL maintainer="Automation System"
LABEL version="1.0.0"
LABEL description="Parallel automation agents for AI-powered development"

# Install basic system dependencies
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

# Set PATH for global npm packages
ENV PATH="/root/.local/bin:${PATH}"

# Install the Kilo Code CLI
RUN npm i -g @kilocode/cli@0.26.0

# Copy package files and install dependencies
COPY package*.json ./
RUN npm install --omit=dev

# Copy application source code
COPY lib/ ./lib/
COPY prompts/ ./prompts/
COPY prompts-prompt/ ./prompts-prompt/
COPY prompts-janitor/ ./prompts-janitor/
COPY prompts-architect/ ./prompts-architect/
COPY run-*.js ./

# Copy .kilocode config to root's home directory
COPY .kilocode /root/.kilocode

# Create necessary directories
RUN mkdir -p /workspace/.state
RUN mkdir -p /app/logs

# Set environment variables
ENV NODE_ENV=production
ENV WORKSPACE_PATH=/workspace
ENV STATE_PATH=/workspace/.state

# Create non-root user for security
RUN groupadd -g 1001 nodejs && \
    useradd -m -u 1001 -g nodejs -s /bin/bash nodejs

# Change ownership of directories
RUN chown -R nodejs:nodejs /app
RUN chown -R nodejs:nodejs /workspace/.state
RUN chown -R nodejs:nodejs /app/logs

# Set the default command
CMD ["node", "run-prompt.js"]

# Health check
HEALTHCHECK --interval=60s --timeout=10s --retries=3 --start-period=30s CMD node -e "console.log('healthy')" || exit 1
