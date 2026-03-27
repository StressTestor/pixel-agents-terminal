mod agent;
mod grid;
mod renderer;
mod scene;
mod sprites;
mod transcript;
mod ui;
mod watcher;

use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "pixel-agents-terminal")]
#[command(version = "0.1.0")]
#[command(about = "Animated pixel-art characters reflecting Claude Code agent state")]
struct Cli {
    /// Root directory to watch for transcript files (default: ~/.claude)
    #[arg(long)]
    project: Option<PathBuf>,

    /// Target frames per second
    #[arg(long, default_value_t = 10)]
    fps: u32,

    /// Display scale multiplier (unused, reserved for future)
    #[arg(long, default_value_t = 1)]
    scale: u32,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Panic hook: restore terminal state before printing the panic
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = crossterm::execute!(
            std::io::stdout(),
            crossterm::terminal::LeaveAlternateScreen,
            crossterm::cursor::Show
        );
        original_hook(info);
    }));

    ui::run(cli.project, cli.fps, cli.scale).await
}
