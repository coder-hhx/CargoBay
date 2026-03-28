FROM python:3.12-slim-bookworm

RUN apt-get update && apt-get install -y --no-install-recommends \
    curl wget git build-essential && \
    rm -rf /var/lib/apt/lists/*

RUN pip install --no-cache-dir numpy pandas requests

WORKDIR /app
CMD ["sleep", "infinity"]
