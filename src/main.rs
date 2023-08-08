use std::{
    fs::File,
    io::{stdin, stdout, BufReader, Read, Write},
    num::NonZeroUsize,
    path::PathBuf,
};

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

        /// The number of threads to spawn to parse and decompress the pbf file.
        ///
        /// Defaults to the number of cores.
        #[arg(short, long)]
        procs: Option<NonZeroUsize>,
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
        Cmd::GetTags { pbf_file, procs } => {
            rayon::ThreadPoolBuilder::new()
                .thread_name(|num| format!("worker{num}"))
                .num_threads(procs.map(usize::from).unwrap_or_default())
                .build_global()?;

            let pbf_file = File::open(pbf_file).map(BufReader::new)?;
            get_tags::run(pbf_file)
        }
        Cmd::Simplify { lang } => {
            let mut input = String::new();
            stdin().read_to_string(&mut input)?;

            let output = om_wikiparser::html::simplify(&input, &lang);

            stdout().write_all(output.as_bytes())?;

            Ok(())
        }
    }
}

/// Get the version returned by `git describe`, e.g.:
/// - `v2.0` if a git tag
/// - the commit hash `034ac04` if not a tag
/// - `034ac04-dirty` if uncommited changes are present,
/// or the crate version if not available (if installed from crates.io).
///
/// See `build.rs` file for more info.
fn version() -> &'static str {
    option_env!("CARGO_GIT_VERSION")
        .or(option_env!("CARGO_PKG_VERSION"))
        .unwrap_or("unknown")
}
