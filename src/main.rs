use std::{
    env,
    fs::File,
    io::{stdin, stdout, BufReader, Read, Write},
    num::NonZeroUsize,
    path::PathBuf,
    str::FromStr,
    thread::available_parallelism,
    time::Instant,
};

use anyhow::Context;
use clap::{CommandFactory, Parser, Subcommand};
#[macro_use]
extern crate log;

mod get_articles;
mod get_tags;

/// Extract articles from Wikipedia Enterprise HTML dumps.
#[derive(Parser)]
#[command(author, version, about, long_about, version = crate::version())]
struct Args {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    GetArticles(get_articles::Args),

    /// Extract wikidata/wikipedia tags from an OpenStreetMap PBF dump.
    ///
    /// Writes to stdout the extracted tags in a TSV format similar to `osmconvert --csv`.
    GetTags {
        /// The `.osm.pbf` file to use.
        pbf_file: PathBuf,

        /// The number of worker threads to spawn to parse and decompress the pbf file.
        ///
        /// If `THREADS` is <= 0, then the number of cores plus `THREADS` threads will be created.
        /// The computed number of threads will never be less than one.
        ///
        /// Defaults to the env variable `OM_POOL_THREADS` if it exists or -2.
        #[arg(short, long, allow_hyphen_values = true)]
        threads: Option<isize>,
    },

    /// Apply the same html article simplification used when extracting articles to stdin, and write it to stdout.
    ///
    /// This is meant for testing and debugging.
    Simplify {
        /// The language to use when processing the article (defaults to `en`).
        #[arg(long, default_value_t = String::from("en"))]
        lang: String,
    },
}

fn main() -> anyhow::Result<()> {
    // Use info level by default, load overrides from `RUST_LOG` env variable.
    // See https://docs.rs/env_logger/latest/env_logger/index.html#example
    env_logger::Builder::new()
        .filter_level(log::LevelFilter::Info)
        .parse_default_env()
        .try_init()?;

    let args = Args::parse();

    info!("{} {}", Args::command().get_name(), version());

    match args.cmd {
        Cmd::GetArticles(args) => {
            if args.wikidata_qids.is_none()
                && args.wikipedia_urls.is_none()
                && args.osm_tags.is_none()
            {
                let mut cmd = Args::command();
                cmd.error(
                    clap::error::ErrorKind::MissingRequiredArgument,
                    "at least one of --osm-tags --wikidata-qids --wikipedia-urls is required",
                )
                .exit()
            }

            get_articles::run(args)
        }
        Cmd::GetTags { pbf_file, threads } => {
            let threads = get_thread_count(threads)
                .context("determining thread count")?
                .get();
            debug!("Using {threads} worker threads");
            rayon::ThreadPoolBuilder::new()
                .thread_name(|num| format!("worker{num}"))
                .num_threads(threads)
                .build_global()
                .context("initializing thread pool")?;
            let pbf_file = File::open(pbf_file).map(BufReader::new)?;
            get_tags::run(pbf_file)
        }
        Cmd::Simplify { lang } => {
            let mut input = String::new();
            stdin().read_to_string(&mut input)?;

            let start = Instant::now();
            let output = om_wikiparser::html::simplify(&input, &lang)?;
            let stop = Instant::now();
            let time = stop.duration_since(start);

            {
                let input_size = input.len() as isize;
                let output_size = output.len() as isize;
                let difference = input_size - output_size;
                let scale = input_size as f64 / output_size as f64;
                info!("Reduced size by {difference} bytes ({scale:.4}x) in {time:?}");
            }

            stdout().write_all(output.as_bytes())?;

            Ok(())
        }
    }
}

/// Determine the number of threads to use.
///
/// If `requested` is <= 0, then the number of cores plus `requested` will be created.
/// If `requested` is `None`, the environment variable `OM_POOL_THREADS` is used, otherwise a default of -2.
/// The computed number of threads will never be less than one.
///
/// # Errors
///
/// Returns an error if:
/// - `OM_POOL_THREADS` is set and cannot be parsed into an isize.
/// - [available_parallelism] returns an error.
fn get_thread_count(requested: Option<isize>) -> anyhow::Result<NonZeroUsize> {
    let env_value = env::var("OM_POOL_THREADS")
        .ok()
        .map(|s| isize::from_str(&s))
        .transpose()
        .context("invalid OM_POOL_THREADS value")?;

    let procs = requested.or(env_value).unwrap_or(-2);
    let procs: usize = if procs > 0 {
        // Explicit thread count.
        procs.try_into().unwrap()
    } else {
        // Relative to cpu count.
        available_parallelism()?
            .get()
            .saturating_sub(procs.abs().try_into().expect("procs.abs() is >= 0"))
    };

    let procs = NonZeroUsize::new(procs).unwrap_or(NonZeroUsize::new(1).unwrap());

    Ok(procs)
}

/// Get the version returned by `git describe`, e.g.:
/// - `v2.0` if a git tag
/// - the commit hash `034ac04` if not a tag
/// - `034ac04-dirty` if uncommited changes are present,
/// or the crate version if not available (debug build or installed from crates.io).
///
/// See `build.rs` file for more info.
fn version() -> &'static str {
    option_env!("CARGO_GIT_VERSION")
        .or(option_env!("CARGO_PKG_VERSION"))
        .unwrap_or("unknown")
}
