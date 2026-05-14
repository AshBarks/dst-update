use clap::{Parser, Subcommand};
use log::error;
use std::time::Duration;

use dst_update::{
    AppError, HtmlUpdateDetail, RSS_URL, UpdateExport, UpdateStatus, clean_html,
    fetch_all_update_details, fetch_latest_update, fetch_rss, latest_rss_update, parse_updates,
};

#[derive(Parser)]
#[command(name = "dst-update", about = "DST 更新查询工具")]
struct Cli {
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,

    #[arg(long)]
    build_only: bool,

    #[arg(long, conflicts_with = "release")]
    test: bool,

    #[arg(long, conflicts_with = "test")]
    release: bool,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    Rss(RssArgs),
    Html(HtmlArgs),
    Export(ExportArgs),
}

#[derive(clap::Args)]
struct RssArgs {
    #[arg(long)]
    detail: bool,

    #[arg(long)]
    build_only: bool,
}

#[derive(clap::Args)]
struct HtmlArgs {
    #[arg(long)]
    build_only: bool,

    #[arg(long, conflicts_with = "release")]
    test: bool,

    #[arg(long, conflicts_with = "test")]
    release: bool,
}

#[derive(clap::Args)]
struct ExportArgs {
    #[arg(long, default_value = "dst_updates.json")]
    output: String,

    #[arg(long)]
    max_pages: Option<u32>,

    #[arg(long)]
    pretty: bool,

    #[arg(long, default_value_t = 5)]
    concurrency: usize,

    #[arg(long, default_value_t = 1000)]
    interval: u64,
}

fn run_rss(args: &RssArgs) -> Result<(), AppError> {
    let channel = fetch_rss(RSS_URL)?;
    let updates = parse_updates(&channel);
    let latest = latest_rss_update(&updates, None).ok_or(AppError::NoUpdateFound)?;

    if args.build_only {
        println!("{}", latest.build_number);
    } else {
        println!(
            "{} {} {}",
            latest.build_number,
            latest.pub_date.to_rfc3339(),
            latest.link
        );
        if args.detail {
            println!("{}", clean_html(&latest.description));
        }
    }

    Ok(())
}

fn run_html(args: &HtmlArgs) -> Result<(), AppError> {
    let status_filter = if args.test {
        Some(UpdateStatus::Test)
    } else if args.release {
        Some(UpdateStatus::Release)
    } else {
        None
    };

    let latest = fetch_latest_update(1, status_filter)?.ok_or(AppError::NoUpdateFound)?;

    if args.build_only {
        println!("{}", latest.build_number);
    } else {
        println!(
            "{} {} {} {}",
            latest.build_number,
            latest.release_date.format("%Y-%m-%d"),
            latest.status,
            latest.link
        );
    }

    Ok(())
}

async fn run_export(args: &ExportArgs) -> Result<(), AppError> {
    let existing = load_existing(&args.output);
    let details = fetch_all_update_details(
        args.max_pages,
        existing,
        args.concurrency,
        Duration::from_millis(args.interval),
    )
    .await?;
    let export = UpdateExport {
        total: details.len(),
        fetched_at: chrono::Utc::now().to_rfc3339(),
        updates: details,
    };
    let json = if args.pretty {
        serde_json::to_string_pretty(&export).unwrap()
    } else {
        serde_json::to_string(&export).unwrap()
    };
    std::fs::write(&args.output, json).map_err(|e| AppError::Io(e.to_string()))?;
    println!("导出 {} 条更新到 {}", export.total, args.output);
    Ok(())
}

fn load_existing(path: &str) -> Option<Vec<HtmlUpdateDetail>> {
    let content = std::fs::read_to_string(path).ok()?;
    let export: UpdateExport = serde_json::from_str(&content).ok()?;
    Some(export.updates)
}

fn main() {
    let cli = Cli::parse();

    let log_level = match cli.verbose {
        0 => "warn",
        1 => "info",
        _ => "debug",
    };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(log_level)).init();

    let result = match cli.command {
        Some(Command::Rss(args)) => run_rss(&args),
        Some(Command::Html(args)) => run_html(&args),
        Some(Command::Export(args)) => {
            let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
            rt.block_on(run_export(&args))
        }
        None => run_html(&HtmlArgs {
            build_only: cli.build_only,
            test: cli.test,
            release: cli.release,
        }),
    };

    if let Err(e) = result {
        error!("{}", e);
        std::process::exit(1);
    }
}
