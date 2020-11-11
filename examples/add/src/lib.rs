use futures::future::JoinAll;
use wasm_bindgen::prelude::*;
use ws_worker::{spawn, spawn_async};

fn add_func() -> i32 {
    let mut sum = 0;
    for i in 1..=1000 {
        sum += i;
    }
    sum
}

#[wasm_bindgen]
pub fn init_worker(url: String) {
    console_error_panic_hook::set_once();
    ws_worker::init(url);
}

#[wasm_bindgen]
pub async fn run_add() -> i32 {
    std::iter::repeat_with(|| spawn_async(add_func))
        .take(10)
        .collect::<JoinAll<_>>()
        .await
        .into_iter()
        .sum()
}

#[wasm_bindgen]
pub async fn run_add_block() -> i32 {
    // need spawn because main thread can't block
    spawn_async(|| {
        std::iter::repeat_with(|| spawn(add_func))
            .take(10)
            .collect::<Vec<_>>()
            .into_iter()
            .map(|j| j.join())
            .sum()
    })
    .await
}
