#![feature(thread_id_value)]

use js_sys::Array;
use once_cell::sync::{Lazy, OnceCell};
use spinning_top::Spinlock;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};
use std::time::Duration;

use wasm_bindgen::prelude::*;
use web_sys::Worker;

#[wasm_bindgen]
pub struct SpawnEntry {
    inner: Box<dyn FnOnce() + Send>,
}

impl SpawnEntry {
    fn new(f: Box<dyn FnOnce() + Send>) -> Self {
        Self { inner: f }
    }

    fn into_inner(self) -> Box<dyn FnOnce() + Send> {
        self.inner
    }
}

struct SpawnEntryList {
    inner: Vec<SpawnEntry>,
    waker: Option<Waker>,
}

impl SpawnEntryList {
    pub fn new() -> Self {
        Self {
            inner: Vec::with_capacity(50),
            waker: None,
        }
    }

    pub fn push(&mut self, entry: SpawnEntry) {
        self.inner.push(entry);
        if let Some(waker) = self.waker.as_ref() {
            waker.wake_by_ref();
        }
    }

    pub fn drain(&mut self) -> impl Iterator<Item = SpawnEntry> + '_ {
        self.inner.drain(..)
    }

    pub fn set_waker(&mut self, waker: Waker) {
        self.waker = Some(waker);
    }
}

static SPAWN_CHANNEL: Lazy<Spinlock<SpawnEntryList>> =
    Lazy::new(|| Spinlock::new(SpawnEntryList::new()));

struct SpawnEntryFuture;

impl Future for SpawnEntryFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut chan = SPAWN_CHANNEL.lock();
        chan.set_waker(cx.waker().clone());

        for entry in chan.drain() {
            post_worker(entry);
        }

        Poll::Pending
    }
}

#[wasm_bindgen]
pub fn ws_thread_spawn_entry() {
    wasm_bindgen_futures::spawn_local(SpawnEntryFuture);
}

#[wasm_bindgen]
pub fn ws_thread_entry(entry: SpawnEntry) {
    entry.into_inner()();
}

fn make_worker_url(source: &str) -> String {
    let base64 = base64::encode(source);
    format!("data:application/javascript;base64,{}", base64)
}

fn make_spawn_worker() -> Worker {
    let worker_url = make_worker_url(include_str!("spawn-worker.js"));
    Worker::new(&worker_url).unwrap()
}

fn make_worker() -> Worker {
    static WORKER_URL: Lazy<String> = Lazy::new(|| make_worker_url(include_str!("worker.js")));

    Worker::new(&WORKER_URL).unwrap()
}

static IMPORT_SCRIPTS: OnceCell<String> = OnceCell::new();

pub fn init(url: String) {
    IMPORT_SCRIPTS.set(url).unwrap();

    let worker = make_spawn_worker();

    let msg = Array::new_with_length(3);
    msg.set(0, wasm_bindgen::module());
    msg.set(1, wasm_bindgen::memory());
    msg.set(
        2,
        IMPORT_SCRIPTS
            .get()
            .expect("Can't get import scripts")
            .into(),
    );
    worker.post_message(msg.as_ref()).unwrap();
}

fn post_worker(entry: SpawnEntry) {
    let worker = make_worker();

    let msg = Array::new_with_length(4);
    msg.set(0, wasm_bindgen::module());
    msg.set(1, wasm_bindgen::memory());
    msg.set(2, entry.into());
    msg.set(
        3,
        IMPORT_SCRIPTS
            .get()
            .expect("Can't get import scripts")
            .into(),
    );
    worker.post_message(msg.as_ref()).unwrap();
}

fn make_handle<F, T>(f: F) -> (JoinHandle<T>, SpawnEntry)
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    let inner = Arc::new(Spinlock::new(HandleState::Empty));

    let entry_fn = {
        let inner = inner.clone();
        move || {
            let ret = f();
            match std::mem::replace(&mut *inner.lock(), HandleState::ThreadFirst(ret)) {
                HandleState::WaitFirst(waker) => waker.wake(),
                _ => {}
            }
        }
    };

    (JoinHandle { inner }, SpawnEntry::new(Box::new(entry_fn)))
}

pub fn spawn<F, T>(f: F) -> JoinHandle<T>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    let (handle, entry) = make_handle(f);

    // Can't use post_worker directly because event loop is stopped during JoinHandle::join
    let mut chan = SPAWN_CHANNEL.lock();
    chan.push(entry);

    handle
}

pub async fn spawn_async<F, T>(f: F) -> T
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    let (handle, entry) = make_handle(f);
    // Use post_worker directly for speed
    post_worker(entry);

    handle.await
}

enum HandleState<T> {
    Empty,
    ThreadFirst(T),
    WaitFirst(Waker),
}

pub struct JoinHandle<T> {
    inner: Arc<Spinlock<HandleState<T>>>,
}

impl<T> Future for JoinHandle<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut inner = self.inner.lock();

        match std::mem::replace(&mut *inner, HandleState::Empty) {
            HandleState::Empty | HandleState::WaitFirst(_) => {
                *inner = HandleState::WaitFirst(cx.waker().clone());
                Poll::Pending
            }
            HandleState::ThreadFirst(val) => Poll::Ready(val),
        }
    }
}

impl<T> JoinHandle<T> {
    pub fn join(mut self) -> T {
        loop {
            if let Some(inner) = Arc::get_mut(&mut self.inner) {
                match std::mem::replace(inner.get_mut(), HandleState::Empty) {
                    HandleState::ThreadFirst(val) => return val,
                    _ => {}
                }
            }

            sleep(Duration::from_millis(100));
        }
    }
}

pub use std::thread::sleep;
