FROM ubuntu:24.04

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        build-essential \
        gcc \
        make \
        patch \
        git \
        ca-certificates \
        curl \
        libx11-dev \
        libxft-dev \
        libxinerama-dev \
        libxrender-dev \
        libfreetype-dev \
        fontconfig \
        xvfb \
        python3 \
        python3-pip \
        python3-pytest \
        python3-coverage \
    && python3 -m pip install --break-system-packages --no-cache-dir ruff==0.17.6 \
    && rm -rf /var/lib/apt/lists/*

COPY rust-orchestrator/target/release/dwm-forge /usr/local/bin/dwm-forge

RUN chmod 0755 /usr/local/bin/dwm-forge

ENTRYPOINT ["dwm-forge"]
