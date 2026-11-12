#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if std::env::args().any(|a| a == "--headless") {
        let runtime = tokio::runtime::Runtime::new().expect("create async runtime");
        if let Err(error) = runtime.block_on(scope_desktop::headless()) {
            eprintln!("Snurpo: {error:#}");
            std::process::exit(1);
        }
    } else {
        scope_desktop::run();
    }
}
