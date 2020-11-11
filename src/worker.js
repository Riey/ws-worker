
self.onmessage = async function(event) {
    const [module, memory, entry, scripts] = event.data;
    importScripts(scripts);
    await wasm_bindgen(module, memory);
    const spawn_entry = wasm_bindgen.SpawnEntry.__wrap(entry.ptr);
    wasm_bindgen.ws_thread_entry(spawn_entry);
    close();
};
