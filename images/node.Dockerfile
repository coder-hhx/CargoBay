FROM node:20-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    curl wget git && \
    rm -rf /var/lib/apt/lists/*

RUN npm install -g pnpm yarn

WORKDIR /app
CMD ["sleep", "infinity"]
