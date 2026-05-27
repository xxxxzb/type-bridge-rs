mod assets;
mod ip;
mod keyboard;
mod server;
mod tray;

use clap::Parser;

#[derive(Parser)]
#[command(name = "type-bridge-rs", version, about = "Wi-Fi remote keyboard — type on your PC from your phone browser")]
struct Cli {
    #[arg(short, long, default_value = "5000")]
    port: u16,
}

fn main() {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    let ip = ip::get_local_ip();
    let port = cli.port;

    println!("\n⌨️  TypeBridge running!");
    println!("📱 Open on your phone: http://{}:{}\n", ip, port);

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

    // Server thread with its own tokio runtime (separate from main thread for macOS tray)
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
        rt.block_on(async { server::run(port, shutdown_rx).await });
    });

    // Tray on main thread (required for macOS). Blocks until Quit.
    tray::run_tray(&ip, port, shutdown_tx);

    // Give the server thread a moment to clean up
    std::thread::sleep(std::time::Duration::from_millis(300));
}
