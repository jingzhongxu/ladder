#!/usr/bin/env bash

set -e

if [ $# -lt 3 ]; then
    echo "Please input seed, port, name!"
    echo "Example : .run.sh 0x00000000000 30333 alice"
    exit 1
fi


KEY=$1
PORT=$2
NODE_NAME=$3
LOG_FILE="$NODE_NAME.log"
PROJECT_ROOT="$( cd "$( dirname "${BASH_SOURCE[0]}" )" >/dev/null && pwd )"
EXE_PATH="$PWD/target/debug/ladder"
BASE_PATH="$PWD/$NODE_NAME"

RUST_LOG='info' $EXE_PATH --chain=ladder --base-path=$BASE_PATH --key=$KEY --name=$NODE_NAME --bootnodes /ip4/192.168.1.109/tcp/30333/p2p/QmYzQGPSnXSXY8PiMBeeeh6e9kHA8j6F7aJdjwWzNxnGUi --port $PORT --validator --telemetry-url ws://telemetry.polkadot.io:1024 > $LOG_FILE 2>&1 & 

echo "Node run with $NODE_NAME, Log in $LOG_FILE file"