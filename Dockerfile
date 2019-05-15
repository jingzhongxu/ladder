FROM ubuntu:18.04
LABEL maintainer="laddernetwork"
LABEL description="This is a docker for ladder node"

WORKDIR /chain

RUN apt-get update \
    && apt-get install -y libssl-dev \
    ca-certificates

EXPOSE 30333 9933 9944
