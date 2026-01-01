# Frontend Dockerfile with Bun and live reload for development
FROM oven/bun:1-alpine

WORKDIR /app

# Install dependencies first (for better caching)
COPY package.json bun.lock ./
RUN bun install --frozen-lockfile

# Copy source files
COPY . .

# Expose Vite dev server port
EXPOSE 3000

# Start Vite dev server with host binding for Docker
CMD ["bun", "run", "dev", "--host", "0.0.0.0"]
