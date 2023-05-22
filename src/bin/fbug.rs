
use clap::Parser;
use tokio::sync::mpsc::unbounded_channel;
use std::path::PathBuf;
use fbug::{config::load_config, state::StateMachine, Event, connections::Connections};
use anyhow::Result;
use std::io::Write;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    #[arg(short, long, default_value = "XDG_CONFIG_HOME/fbug/config.yaml")]
    pub config_path: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    setup_logging();
    let args = Args::parse();
    let device = load_config(&args.config_path).unwrap();
    let sm = StateMachine::new(device.states, device.transitions)?;
    let triggers = sm.list_triggers();
    
    for trigger in triggers {
        println!("{}", trigger);
    }

    let (tx, rx) = unbounded_channel::<Event>();
    let connections = Connections::new(tx, &device.connections).await?;

    Ok(())
}

fn setup_logging() {
    #[cfg(debug_assertions)]
    ::std::env::set_var("RUST_LOG", "trace");
    #[cfg(not(debug_assertions))]
    ::std::env::set_var("RUST_LOG", "info");

    env_logger::Builder::from_default_env()
        .format(|buf, record| {
            let style = buf.default_level_style(record.level());

            writeln!(
                buf,
                "{} {:<10} [{}] {}",
                chrono::Local::now().format("%F %T%.3f"),
                record
                    .module_path()
                    .unwrap_or("")
                    .split("::")
                    .last()
                    .unwrap_or(""),
                style.value(record.level()),
                record.args()
            )
        })
        .init();
}
