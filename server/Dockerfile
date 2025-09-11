FROM debian:stable-slim

RUN apt-get update \
    && export DEBIAN_FRONTEND=noninteractive \
    && apt-get install -y --no-install-recommends --assume-yes \
        ca-certificates \
        librust-backtrace+serde-dev \
        libssl-dev \
    && rm -rf /var/lib/apt/lists/*;

WORKDIR /opt

COPY ./airshipper-server /opt/airshipper-server

ENV RUST_BACKTRACE=full
EXPOSE 8000
CMD [ "/opt/airshipper-server" ]
