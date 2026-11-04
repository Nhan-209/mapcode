mod service;

use service::Service;

fn run_pipeline() {
    let s = Service::new("auth");
    let res = s.process();
    s.close();
}

fn main() {
    run_pipeline();
}
