Substrate Node for Ladder network

## Hacking

1. environment

Refer to [Substrate](https://github.com/paritytech/substrate#building)

2. build

Then build the code:

```
./build.sh

cargo build
```

You can start a development chain with:
```
cargo run -- --dev
```

## Docker
build by cargo, then create a link in OUT_DIR.
```
ln -s ./target/debug/docker.sh ./docker/docker.sh
```
run it 
```
cd ./target/debug
./docker.sh
./ladder --dev
```

## UI
* visit https://polkadot.js.org/apps/ .
* Settings => Local Node.
