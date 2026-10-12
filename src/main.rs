use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use humpyard::config::Config;
use humpyard::prices;
use humpyard::server;

#[derive(Parser)]
#[command(version, about = "Unified LLM gateway")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Start the gateway.
    Serve {
        #[arg(short, long)]
        config: PathBuf,
    },
    /// Validate a config file without starting the server.
    CheckConfig { path: PathBuf },
    /// Create a virtual key: prints the key once and the config block holding its hash.
    Keygen { id: String },
    /// Propose prices from a public source, or check a config's prices for drift. Reads a source
    /// over the network and never changes the config or a running gateway.
    Prices {
        #[command(subcommand)]
        command: PricesCommand,
    },
}

#[derive(clap::Args)]
struct PriceArgs {
    /// The config file whose endpoints are priced.
    #[arg(short, long)]
    config: PathBuf,
    /// Where prices come from: `models-dev:<provider id>`, for example `models-dev:opencode-go`.
    #[arg(short, long)]
    source: String,
    /// Which provider of the config the source describes (its name in `[providers.<name>]`).
    #[arg(short, long)]
    provider: String,
    /// Override the source URL (testing); https only, or http to a loopback host.
    #[arg(long, hide = true, default_value = prices::MODELS_DEV_URL)]
    url: String,
}

#[derive(Subcommand)]
enum PricesCommand {
    /// Print the `price` entries the source proposes for the provider's endpoints.
    Import(PriceArgs),
    /// Report endpoints whose price differs from, is missing in, or is gone from the source.
    /// Exit code 0: no drift, 1: drift, 2: the source or config could not be read.
    Check {
        #[command(flatten)]
        args: PriceArgs,
        /// Print the report as JSON.
        #[arg(long)]
        json: bool,
    },
}

/// Exit code for a source or config that could not be used (distinct from drift).
const EXIT_UNUSABLE: u8 = 2;

async fn load_prices(args: &PriceArgs) -> Result<(Vec<prices::Row>, String), prices::PriceError> {
    let provider_id = prices::parse_source(&args.source)?;
    // Keys are not needed to compare prices: any value satisfies the env lookup.
    let text = std::fs::read_to_string(&args.config)
        .map_err(|e| prices::PriceError::Config(format!("{}: {e}", args.config.display())))?;
    let config = Config::from_toml(&text, |_| Some("unused".into()))
        .map_err(|e| prices::PriceError::Config(e.to_string()))?;
    if !config.providers.contains_key(&args.provider) {
        return Err(prices::PriceError::Config(format!(
            "the config has no provider `{}`",
            args.provider
        )));
    }
    let data = prices::parse_models_dev(&prices::fetch(&args.url).await?, provider_id)?;
    Ok((prices::compare(&config, &args.provider, &data), data.source))
}

async fn prices_command(command: PricesCommand) -> ExitCode {
    let (args, json, check) = match command {
        PricesCommand::Import(args) => (args, false, false),
        PricesCommand::Check { args, json } => (args, json, true),
    };
    let read_on = time::OffsetDateTime::now_utc().date().to_string();
    let (rows, source) = match load_prices(&args).await {
        Ok(loaded) => loaded,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::from(EXIT_UNUSABLE);
        }
    };
    if !check {
        print!("{}", prices::render_snippet(&rows, &source, &read_on));
        return ExitCode::SUCCESS;
    }
    if json {
        println!("{}", prices::render_json(&rows, &source, &read_on));
    } else {
        print!("{}", prices::render_report(&rows, &source, &read_on));
    }
    if prices::has_drift(&rows) {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

/// Turns SIGINT (Ctrl-C) and, on Unix, SIGTERM into messages: the first starts a graceful
/// shutdown, a second ends the wait for in-flight requests.
fn termination_signals() -> tokio::sync::mpsc::Receiver<()> {
    let (tx, rx) = tokio::sync::mpsc::channel(4);
    tokio::spawn(async move {
        #[cfg(unix)]
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).ok();
        loop {
            #[cfg(unix)]
            {
                let sigterm = async {
                    match terminate.as_mut() {
                        Some(signal) => {
                            signal.recv().await;
                        }
                        None => std::future::pending::<()>().await,
                    }
                };
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {}
                    () = sigterm => {}
                }
            }
            #[cfg(not(unix))]
            {
                let _ = tokio::signal::ctrl_c().await;
            }
            if tx.send(()).await.is_err() {
                break;
            }
        }
    });
    rx
}

/// Turns SIGHUP into reload requests: each one makes the running gateway re-read its config file.
/// `None` where the platform has no SIGHUP.
fn reload_signals() -> Option<tokio::sync::mpsc::Receiver<()>> {
    #[cfg(unix)]
    {
        let mut hangup =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::hangup()).ok()?;
        let (tx, rx) = tokio::sync::mpsc::channel(4);
        tokio::spawn(async move {
            while hangup.recv().await.is_some() {
                // A full queue already has reloads pending, which cover this one.
                let _ = tx.try_send(());
            }
        });
        Some(rx)
    }
    #[cfg(not(unix))]
    {
        None
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();
    let command = Cli::parse().command;
    if let Command::Prices { command } = command {
        return prices_command(command).await;
    }
    match run(command).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
    }
}

async fn run(command: Command) -> Result<(), String> {
    match command {
        Command::CheckConfig { path } => {
            Config::load(&path).map_err(|e| e.to_string())?;
            println!("config ok");
            Ok(())
        }
        Command::Keygen { id } => {
            let (key, hash) = humpyard::auth::generate_key()
                .map_err(|e| format!("cannot gather randomness: {e}"))?;
            println!("Key (shown once; store it securely): {key}\n");
            println!("Add to your config:\n\n[keys.{id}]\nsha256 = \"{hash}\"");
            Ok(())
        }
        Command::Prices { .. } => Ok(()), // handled in `main`, where it chooses its exit code
        Command::Serve { config: path } => {
            let config = Config::load(&path).map_err(|e| e.to_string())?;
            let listener = tokio::net::TcpListener::bind(config.listen)
                .await
                .map_err(|e| format!("cannot bind {}: {e}", config.listen))?;
            tracing::info!(listen = %config.listen, "serving");
            let reload = reload_signals().map(|triggers| server::Reload { path, triggers });
            server::run(config, listener, termination_signals(), reload).await
        }
    }
}
