use std::{
    collections::HashSet,
    env,
    fs::File,
    io::{stderr, stdin, stdout, BufReader, IsTerminal, Read, Write},
    num::NonZeroUsize,
    path::PathBuf,
    process,
    str::FromStr,
    thread::available_parallelism,
    time::Instant,
};

use anyhow::Context;
use clap::{CommandFactory, Parser, Subcommand};
#[macro_use]
extern crate tracing;
use tracing_subscriber::filter::EnvFilter;

use om_wikiparser::osm;

mod get_articles;
mod get_tags;

/// A set of tools to extract articles from Wikipedia Enterprise HTML dumps selected by OpenStreetMap tags.
#[derive(Parser)]
#[command(author, version, about, long_about, version = crate::version())]
struct Args {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Extract wikidata/wikipedia tags from an OpenStreetMap PBF dump.
    ///
    /// Writes to stdout the extracted tags in a TSV format similar to `osmconvert --csv`.
    /// Unlike `osmconvert`, this **does not** truncate long tag values and create invalid UTF-8.
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

    /// Attempt to parse extracted OSM tags and write errors to stdout in TSV format.
    CheckTags {
        /// Path to a TSV file that contains one or more of `wikidata`, `wikipedia` columns.
        ///
        /// This can be generated with the `get-tags` command or `osmconvert --csv-headline --csv 'wikidata wikipedia'`.
        /// If `@id`, `@version`, and `@otype` or `@oname` columns are present, they will be added to the output for additional context.
        #[arg(value_name = "FILE.tsv")]
        osm_tags: PathBuf,
    },

    /// Extract, filter, and simplify article HTML from Wikipedia Enterprise HTML dumps.
    ///
    /// Expects an uncompressed dump (newline-delimited JSON) connected to stdin.
    GetArticles(get_articles::Args),

    /// Apply html simplification to a single article.
    ///
    /// Reads from stdin and writes the simplified version to stdout.
    /// This is meant for testing and debugging.
    Simplify {
        /// The language to use when processing the article (tries to detect it by default, falling back to `en`).
        #[arg(long)]
        lang: Option<String>,
    },
}

fn main() -> anyhow::Result<()> {
    // Use info level by default, load overrides from `RUST_LOG` env variable.
    // See https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(stderr);

    if stderr().is_terminal() {
        subscriber.compact().init();
    } else {
        subscriber
            .json()
            .flatten_event(true)
            .with_current_span(false)
            .with_span_list(true)
            .init();
    }

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

            let pid = process::id();
            let span = info_span!("", pid);
            let _handle = span.enter();
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
        Cmd::CheckTags { osm_tags } => {
            let mut qids = HashSet::new();
            let mut titles = HashSet::new();
            let mut errors = Vec::new();
            info!("Reading osm tag file");
            om_wikiparser::parse_osm_tag_file(osm_tags, &mut qids, &mut titles, Some(&mut errors))?;
            info!("Found {} errors in tag file", errors.len());

            let mut writer = csv::WriterBuilder::new()
                .delimiter(b'\t')
                .from_writer(stdout().lock());

            writer.write_record(["line", "object", "version", "key", "error", "value"])?;

            for error in errors {
                use om_wikiparser::ParseErrorKind::*;
                let key = match error.kind {
                    Title(_) => "wikipedia",
                    Qid(_) => "wikidata",
                    Tsv(_) => "",
                };

                // Url or id.
                let object = error
                    .osm_id
                    .map(|id| {
                        error
                            .osm_type
                            .and_then(|obj| osm::make_url(obj, id))
                            .unwrap_or_else(|| id.to_string())
                    })
                    .unwrap_or_default();

                let version = error.osm_version.map(|v| v.to_string()).unwrap_or_default();

                // Capture error chain.
                let e: anyhow::Error = match error.kind {
                    Title(e) => e.into(),
                    Qid(e) => e.into(),
                    Tsv(e) => e.into(),
                };
                let msg = format!("{:#}", e);

                writer.write_record([
                    &error.line.to_string(),
                    &object,
                    &version,
                    key,
                    &msg,
                    &error.text,
                ])?;
            }

            Ok(())
        }
        Cmd::Simplify { lang } => {
            use om_wikiparser::html;

            let mut input = String::new();
            stdin().read_to_string(&mut input)?;

            let document = scraper::Html::parse_document(&input);

            let lang = lang.unwrap_or_else(|| match html::detect_lang(&document) {
                Some(detected) => {
                    info!("Detected language as {detected:?}");
                    detected
                }
                None => {
                    warn!("Unable to detect language, assuming \"en\"");
                    "en".to_string()
                }
            });

            let start = Instant::now();
            let output = html::process(document, &lang)?.html();
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
