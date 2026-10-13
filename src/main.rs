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
    /// Which provider of the config to check (its name in `[providers.<name>]`). Without it, every
    /// provider that sets `price_source` is checked.
    #[arg(short, long)]
    provider: Option<String>,
    /// Where prices come from, `models-dev:<provider id>`, overriding the provider's
    /// `price_source`. Needs `--provider`.
    #[arg(short, long)]
    source: Option<String>,
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

/// The (provider, source) pairs to compare: what the flags name, or every provider of the config
/// that sets a `price_source`.
fn price_plan(
    args: &PriceArgs,
    config: &Config,
) -> Result<Vec<(String, String)>, prices::PriceError> {
    let fail = |message: String| Err(prices::PriceError::Config(message));
    match (&args.provider, &args.source) {
        (None, Some(_)) => fail("--source needs --provider".into()),
        (Some(provider), source) => {
            let Some(configured) = config.providers.get(provider) else {
                return fail(format!("the config has no provider `{provider}`"));
            };
            match source.as_ref().or(configured.price_source.as_ref()) {
                Some(source) => Ok(vec![(provider.clone(), source.clone())]),
                None => fail(format!(
                    "provider `{provider}` has no `price_source`; set one in the config or pass --source"
                )),
            }
        }
        (None, None) => {
            let plan: Vec<_> = config
                .providers
                .iter()
                .filter_map(|(name, p)| Some((name.clone(), p.price_source.clone()?)))
                .collect();
            if plan.is_empty() {
                return fail(
                    "no provider sets `price_source`; set one in the config, or pass --provider and --source"
                        .into(),
                );
            }
            Ok(plan)
        }
    }
}

async fn load_prices(args: &PriceArgs) -> Result<Vec<prices::Report>, prices::PriceError> {
    // Keys are not needed to compare prices: any value satisfies the env lookup.
    let text = std::fs::read_to_string(&args.config)
        .map_err(|e| prices::PriceError::Config(format!("{}: {e}", args.config.display())))?;
    let config = Config::from_toml(&text, |_| Some("unused".into()))
        .map_err(|e| prices::PriceError::Config(e.to_string()))?;
    let plan = price_plan(args, &config)?;
    // Validate every source before fetching anything, then fetch the document once.
    let ids = plan
        .iter()
        .map(|(_, spec)| prices::parse_source(spec))
        .collect::<Result<Vec<_>, _>>()?;
    let body = prices::fetch(&args.url).await?;
    plan.iter()
        .zip(ids)
        .map(|((provider, _), id)| {
            let data = prices::parse_models_dev(&body, id)?;
            Ok(prices::Report {
                provider: provider.clone(),
                rows: prices::compare(&config, provider, &data),
                source: data.source,
            })
        })
        .collect()
}

async fn prices_command(command: PricesCommand) -> ExitCode {
    let (args, json, check) = match command {
        PricesCommand::Import(args) => (args, false, false),
        PricesCommand::Check { args, json } => (args, json, true),
    };
    let read_on = time::OffsetDateTime::now_utc().date().to_string();
    let reports = match load_prices(&args).await {
        Ok(reports) => reports,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::from(EXIT_UNUSABLE);
        }
    };
    if !check {
        for report in &reports {
            print!(
                "{}",
                prices::render_snippet(&report.rows, &report.source, &read_on)
            );
        }
        return ExitCode::SUCCESS;
    }
    if json {
        println!("{}", prices::render_json(&reports, &read_on));
    } else {
        for report in &reports {
            print!(
                "{}",
                prices::render_report(&report.rows, &report.source, &read_on)
            );
        }
    }
    if reports.iter().any(|r| prices::has_drift(&r.rows)) {
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
