function ws_worker_new() {
    let worker = new Worker("worker.js", {name: self.toString()});
    return worker;
}
