# Custom PostgreSQL image with pgvector and pg_textsearch
FROM pgvector/pgvector:pg18-trixie

# Install build dependencies
RUN apt-get update && apt-get install -y \
    build-essential \
    git \
    postgresql-server-dev-18 \
    libpq-dev \
    curl \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Clone and build pg_textsearch
WORKDIR /tmp
RUN git clone https://github.com/timescale/pg_textsearch.git && \
    cd pg_textsearch && \
    make && \
    make install && \
    cd .. && \
    rm -rf pg_textsearch

# Clean up build dependencies to reduce image size
RUN apt-get purge -y --auto-remove \
    build-essential \
    git \
    postgresql-server-dev-18 \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*

# Switch back to postgres user
USER postgres

# Set working directory
WORKDIR /var/lib/postgresql
