mod config;
mod layout;
mod runtime;

use crate::config::Config;
use crate::layout::RenderLayout;
use crate::runtime::{GStreamerBackend, StreamManager};
use std::error::Error;
use std::path::PathBuf;
use std::time::Duration;

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let config_path = match args.as_slice() {
        [flag, path] if flag == "--config" => Some(PathBuf::from(path)),
        [path] => Some(PathBuf::from(path)),
        [] => {
            let default = PathBuf::from("config/camera-wall.example.toml");
            if default.exists() {
                Some(default)
            } else {
                None
            }
        }
        _ => {
            eprintln!("usage: PiViewWall [--config <path>]");
            std::process::exit(1);
        }
    };

    let config = match config_path {
        Some(path) => Config::from_file(&path)?,
        None => {
            eprintln!("No configuration file was provided and the default example config was not found.");
            std::process::exit(1);
        }
    };

    config.validate().map_err(|err| format!("invalid config: {err}"))?;
    config.print_summary();

    let render_layout = RenderLayout::from_config(&config)
        .map_err(|err| format!("invalid display layout: {err}"))?;
    render_layout.render_summary();

    let mut stream_manager = StreamManager::from_config(&config);
    stream_manager
        .launch_with_backend(&GStreamerBackend, &render_layout)
        .map_err(|err| format!("failed to launch streams: {err}"))?;
    stream_manager.print_status();

    println!("Starting watcher loop. Press Ctrl+C to stop.");
    stream_manager
        .run_service_loop(Duration::from_secs(5))
        .map_err(|err| format!("failed while monitoring stream health: {err}"))?;

    Ok(())
}
