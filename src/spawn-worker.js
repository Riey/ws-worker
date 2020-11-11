self.onmessage = async function(event) {
    const [module, memory, scripts] = event.data;
    importScripts(scripts);
    await wasm_bindgen(module, memory);
    wasm_bindgen.ws_thread_spawn_entry();
}
