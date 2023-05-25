use anyhow::Result;
use clap::Parser;
use env_logger::fmt::Formatter;
use fbug::main_loop;
use fbug::{config::load_config, connections::Connections, state::StateMachine, Event};
use log::Record;
use std::io::Write;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    // TODO: Have main conf + multiple per device configs
    #[arg(short, long, default_value = "XDG_CONFIG_HOME/fbug/config.yaml")]
    pub config_path: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    setup_logging();
    let args = Args::parse();
    let device = load_config(&args.config_path).unwrap();

    main_loop(device).await
}

fn setup_logging() {
    #[cfg(debug_assertions)]
    ::std::env::set_var("RUST_LOG", "trace");
    #[cfg(not(debug_assertions))]
    ::std::env::set_var("RUST_LOG", "info");

    env_logger::Builder::from_default_env()
        .format(|buf, record| {
            let style = buf.default_level_style(record.level());
            let mut local_file_style = buf.style();
            let p = PathBuf::from(record.file().unwrap_or(""));

            let p = if p.is_absolute() {
                local_file_style.set_color(env_logger::fmt::Color::Cyan);
                p.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
            } else {
                local_file_style.set_color(env_logger::fmt::Color::Green);
                p.to_string_lossy()
            };

            let p = if record.target().starts_with("device:") {
                format!("{}", record.target().split(":").last().unwrap())
            } else {
                format!("{} at {}:{}:",
                        record
                            .module_path()
                            .unwrap_or(""),
                    p,
                    record.line().unwrap_or(0))
            };

            write!(
                buf,
                "[{:<5}] {} │ {} {}\n",
                style.value(record.level()),
                chrono::Local::now().format("%T%.3f"),
                local_file_style.value(p),
                record.args()
            )
        })
        .init();
}
