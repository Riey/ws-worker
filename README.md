# ws-worker
spawn worker in rust wasm

## Prerequirements

1. need rust flags "-Ctarget-feature=+atomics,+bulk-memory"
2. need unstable cargo flags "-Zbuild-std=std,panic_abort"

These two can be easily set to .cargo/config see [this repo's configuration](https://github.com/Riey/ws-worker/blob/master/.cargo/config)

3. need SharedArrayBuffer enabled browser

please checkout [caniuse](https://caniuse.com/sharedarraybuffer)

firefox can technically support but I only tested on chrome

## Usage

```rust
use ws_worker::{init, spawn, spawn_async};

// you must run init function first
// it needed for loading wasm-bindgen scripts in worker
init("http://localhost/pkg/add.js");

pub async fn run() {
  assert_eq!(spawn_async(|| 1 + 2).await, 3);
}

pub fn run_blocking() {
  assert_eq!(spawn(|| 1 + 2), 3);
}
```

This library support not only async but also blocking api but don't use blocking function in main thread
