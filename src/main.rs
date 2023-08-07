use clap::{CommandFactory, Parser, Subcommand};
#[macro_use]
extern crate log;

mod get_articles;

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

#[derive(Parser)]
#[command(version = crate::version())]
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
    GetTags,

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
            if args.wikidata_ids.is_none()
                && args.wikipedia_urls.is_none()
                && args.osm_tags.is_none()
            {
                let mut cmd = Args::command();
                cmd.error(
                    clap::error::ErrorKind::MissingRequiredArgument,
                    "at least one of --osm-tags --wikidata-ids --wikipedia-urls is required",
                )
                .exit()
            }

            get_articles::run(args)
        }
        Cmd::GetTags => todo!(),
        Cmd::Simplify { lang } => {
            use std::io::{stdin, stdout, Read, Write};

            let mut input = String::new();
            stdin().read_to_string(&mut input)?;

            let output = om_wikiparser::html::simplify(&input, &lang);

            stdout().write_all(output.as_bytes())?;

            Ok(())
        }
    }
}
