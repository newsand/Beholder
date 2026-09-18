mod app;

use beholder_core::mcp::McpServer;
use beholder_core::nvml::try_init_nvml;
use clap::Parser;

#[derive(Parser)]
#[command(name = "beholder")]
#[command(about = "Process and VRAM monitor for Linux")]
#[command(version)]
struct Cli {
    #[arg(long, help = "Run in MCP headless mode (stdio)")]
    mcp: bool,
}

fn main() {
    let cli = Cli::parse();
    let nvml = try_init_nvml();

    if cli.mcp {
        let mut server = McpServer::new(nvml);
        server.run();
    } else {
        let options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([1200.0, 800.0])
                .with_min_inner_size([800.0, 600.0]),
            ..Default::default()
        };

        if let Err(e) = eframe::run_native(
            "Beholder",
            options,
            Box::new(|cc| Ok(Box::new(app::BeholderApp::new(cc, nvml)))),
        ) {
            eprintln!("Error running GUI: {}", e);
            std::process::exit(1);
        }
    }
}
