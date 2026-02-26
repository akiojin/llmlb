//! LLM load balancer Server Entry Point

use clap::Parser;
use llmlb::cli::{Cli, Commands};
use llmlb::config::ServerConfig;
use llmlb::logging;

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn main() {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => match e.kind() {
            clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion => {
                e.exit();
            }
            _ => {
                e.exit();
            }
        },
    };

    // Handle subcommands
    match cli.command {
        Some(Commands::Internal(args)) => {
            logging::init().expect("failed to initialize logging");
            if let Err(e) = llmlb::cli::internal::execute(args.command) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
            return;
        }
        Some(Commands::Stop(args)) => {
            let runtime = tokio::runtime::Runtime::new().expect("Failed to create runtime");
            if let Err(e) = runtime.block_on(llmlb::cli::stop::execute(&args)) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
            return;
        }
        Some(Commands::Status(args)) => {
            let runtime = tokio::runtime::Runtime::new().expect("Failed to create runtime");
            if let Err(e) = runtime.block_on(llmlb::cli::status::execute(&args)) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
            return;
        }
        Some(Commands::Assistant(args)) => {
            let runtime = tokio::runtime::Runtime::new().expect("Failed to create runtime");
            if let Err(e) = runtime.block_on(llmlb::cli::assistant::execute(&args.command)) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
            return;
        }
        Some(Commands::Serve(args)) => {
            logging::init().expect("failed to initialize logging");
            use llmlb::gui::tray::{run_with_system_tray, TrayOptions};
            use std::thread;
            use tokio::runtime::Builder;

            let config = ServerConfig::from_args(args.host, args.port);
            if args.no_tray {
                let runtime = Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .expect("failed to build Tokio runtime for server mode");
                runtime.block_on(run_server(config, None));
                return;
            }
            let tray_options = TrayOptions::new(&config.base_url(), &config.dashboard_url());
            let fallback_config = config.clone();

            let result = run_with_system_tray(tray_options, move |proxy| {
                let server_config = config.clone();
                thread::spawn(move || {
                    let runtime = Builder::new_multi_thread()
                        .enable_all()
                        .build()
                        .expect("failed to build Tokio runtime for system tray mode");
                    runtime.block_on(run_server(server_config, Some(proxy.clone())));
                    proxy.notify_server_exit();
                });
            });
            if let Err(err) = result {
                tracing::error!("System tray initialization failed: {err}");
                let runtime = Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .expect("failed to build Tokio runtime for server mode");
                runtime.block_on(run_server(fallback_config, None));
            }
            return;
        }
        None => {
            // No subcommand - use default GUI mode
        }
    }

    logging::init().expect("failed to initialize logging");
    use llmlb::gui::tray::{run_with_system_tray, TrayOptions};
    use std::thread;
    use tokio::runtime::Builder;

    let config = ServerConfig::from_env();
    let tray_options = TrayOptions::new(&config.base_url(), &config.dashboard_url());
    let fallback_config = config.clone();

    let result = run_with_system_tray(tray_options, move |proxy| {
        let server_config = config.clone();
        thread::spawn(move || {
            let runtime = Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("failed to build Tokio runtime for system tray mode");
            runtime.block_on(run_server(server_config, Some(proxy.clone())));
            proxy.notify_server_exit();
        });
    });
    if let Err(err) = result {
        tracing::error!("System tray initialization failed: {err}");
        let runtime = Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("failed to build Tokio runtime for server mode");
        runtime.block_on(run_server(fallback_config, None));
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // Handle subcommands
    match cli.command {
        Some(Commands::Internal(args)) => {
            logging::init().expect("failed to initialize logging");
            if let Err(e) = llmlb::cli::internal::execute(args.command) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
            return;
        }
        Some(Commands::Stop(args)) => {
            if let Err(e) = llmlb::cli::stop::execute(&args).await {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
            return;
        }
        Some(Commands::Status(args)) => {
            if let Err(e) = llmlb::cli::status::execute(&args).await {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
            return;
        }
        Some(Commands::Assistant(args)) => {
            if let Err(e) = llmlb::cli::assistant::execute(&args.command).await {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
            return;
        }
        Some(Commands::Serve(args)) => {
            logging::init().expect("failed to initialize logging");
            let cfg = ServerConfig::from_args(args.host, args.port);
            run_server(cfg).await;
            return;
        }
        None => {
            // No subcommand - default to serve
            logging::init().expect("failed to initialize logging");
            let cfg = ServerConfig::from_env();
            run_server(cfg).await;
        }
    }
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
async fn run_server(config: ServerConfig, tray_proxy: Option<llmlb::gui::tray::TrayEventProxy>) {
    let ctx = llmlb::bootstrap::initialize(config.port, tray_proxy).await;
    llmlb::server::run(ctx.state, &config.bind_addr()).await;
    // ctx._server_lock はここでDropされ、ロックが解除される
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
async fn run_server(config: ServerConfig) {
    let ctx = llmlb::bootstrap::initialize(config.port).await;
    llmlb::server::run(ctx.state, &config.bind_addr()).await;
    // ctx._server_lock はここでDropされ、ロックが解除される
}
