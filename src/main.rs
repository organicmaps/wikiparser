use std::{
    fs::{self, File},
    io::{stdin, BufRead, Write},
    os::unix,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, bail, Context};
use clap::{CommandFactory, Parser};
#[macro_use]
extern crate log;

use om_wikiparser::{
    html::simplify,
    wm::{parse_osm_tag_file, parse_wikidata_file, parse_wikipedia_file, Page, WikipediaTitleNorm},
};

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

/// Extract article HTML from Wikipedia Enterprise HTML dumps.
///
/// Expects an uncompressed dump connected to stdin.
#[derive(Parser)]
#[command(version = crate::version())]
struct Args {
    /// Directory to write the extracted articles to.
    output_dir: PathBuf,

    /// Path to a TSV file that contains one or more of `wikidata`, `wikipedia` columns.
    ///
    /// This can be generated with `osmconvert --csv-headline --csv 'wikidata wikipedia'`.
    #[arg(long, help_heading = "FILTERS")]
    osm_tags: Option<PathBuf>,

    /// Path to file that contains a Wikidata QID to extract on each line
    /// (e.g. `Q12345`).
    #[arg(long, help_heading = "FILTERS")]
    wikidata_ids: Option<PathBuf>,

    /// Path to file that contains a Wikipedia article url to extract on each line
    /// (e.g. `https://lang.wikipedia.org/wiki/Article_Title`).
    #[arg(long, help_heading = "FILTERS")]
    wikipedia_urls: Option<PathBuf>,

    /// Append to the provided file path the QIDs of articles matched by title but not QID.
    ///
    /// Use this to save the QIDs of articles you know the url of, but not the QID.
    /// The same path can later be passed to the `--wikidata-ids` option to extract them from another language's dump.
    /// Writes are atomicly appended to the file, so the same path may be used by multiple concurrent instances.
    #[arg(long, requires("wikipedia_urls"))]
    write_new_ids: Option<PathBuf>,
}

/// Determine the directory to write the article contents to, create it, and create any necessary symlinks to it.
fn create_article_dir(
    base: impl AsRef<Path>,
    page: &Page,
    redirects: impl IntoIterator<Item = WikipediaTitleNorm>,
) -> anyhow::Result<PathBuf> {
    let base = base.as_ref();
    let mut redirects = redirects.into_iter();

    let main_dir = match page.wikidata() {
        None => {
            // Write to wikipedia title directory.
            // Prefer first redirect, fall back to page title if none exist
            info!("Page without wikidata qid: {:?} ({})", page.name, page.url);
            redirects
                .next()
                .or_else(|| match page.title() {
                    Ok(title) => Some(title),
                    Err(e) => {
                        warn!("Unable to parse title for page {:?}: {:#}", page.name, e);
                        None
                    }
                })
                // hard fail when no titles can be parsed
                .ok_or_else(|| anyhow!("No available titles for page {:?}", page.name))?
                .get_dir(base.to_owned())
        }
        Some(qid) => {
            // Otherwise use wikidata as main directory and symlink from wikipedia titles.
            qid.get_dir(base.to_owned())
        }
    };

    if main_dir.is_symlink() {
        fs::remove_file(&main_dir)
            .with_context(|| format!("removing old link for main directory {:?}", &main_dir))?;
    }
    fs::create_dir_all(&main_dir)
        .with_context(|| format!("creating main directory {:?}", &main_dir))?;

    // Write symlinks to main directory.
    for title in redirects {
        let wikipedia_dir = title.get_dir(base.to_owned());

        // Build required directory.
        //
        // Possible states from previous run:
        // - Does not exist (and is not a symlink)
        // - Exists, is a directory
        // - Exists, is a valid symlink to correct location
        // - Exists, is a valid symlink to incorrect location
        if wikipedia_dir.exists() {
            if wikipedia_dir.is_symlink() {
                // Only replace if not valid
                if fs::read_link(&wikipedia_dir)? == main_dir {
                    continue;
                }
                fs::remove_file(&wikipedia_dir)?;
            } else {
                fs::remove_dir_all(&wikipedia_dir)?;
            }
        } else {
            // titles can contain `/`, so ensure necessary subdirs exist
            let parent_dir = wikipedia_dir.parent().unwrap();
            fs::create_dir_all(parent_dir)
                .with_context(|| format!("creating wikipedia directory {:?}", parent_dir))?;
        }

        unix::fs::symlink(&main_dir, &wikipedia_dir).with_context(|| {
            format!(
                "creating symlink from {:?} to {:?}",
                wikipedia_dir, main_dir
            )
        })?;
    }

    Ok(main_dir)
}

/// Write selected article to disk.
///
/// - Write page contents to wikidata page (`wikidata.org/wiki/QXXX/lang.html`).
/// - If the page has no wikidata qid, write contents to wikipedia location (`lang.wikipedia.org/wiki/article_title/lang.html`).
/// - Create links from all wikipedia urls and redirects (`lang.wikipedia.org/wiki/a_redirect -> wikidata.org/wiki/QXXX`).
fn write(
    base: impl AsRef<Path>,
    page: &Page,
    redirects: impl IntoIterator<Item = WikipediaTitleNorm>,
) -> anyhow::Result<()> {
    let article_dir = create_article_dir(base, page, redirects)?;

    // Write html to determined file.
    let mut filename = article_dir;
    filename.push(&page.in_language.identifier);
    filename.set_extension("html");

    debug!("{:?}: {:?}", page.name, filename);

    if filename.exists() {
        debug!("Overwriting existing file");
    }

    let html = simplify(&page.article_body.html, &page.in_language.identifier);

    let mut file =
        File::create(&filename).with_context(|| format!("creating html file {:?}", filename))?;
    file.write_all(html.as_bytes())
        .with_context(|| format!("writing html file {:?}", filename))?;

    Ok(())
}

fn main() -> anyhow::Result<()> {
    // Use info level by default, load overrides from `RUST_LOG` env variable.
    // See https://docs.rs/env_logger/latest/env_logger/index.html#example
    env_logger::Builder::new()
        .filter_level(log::LevelFilter::Info)
        .parse_default_env()
        .try_init()?;

    let args = Args::parse();

    if args.wikidata_ids.is_none() && args.wikipedia_urls.is_none() && args.osm_tags.is_none() {
        let mut cmd = Args::command();
        cmd.error(
            clap::error::ErrorKind::MissingRequiredArgument,
            "at least one --osm-tags --wikidata-ids --wikipedia-urls is required",
        )
        .exit()
    }

    info!("{} {}", Args::command().get_name(), version());

    let mut wikipedia_titles = if let Some(path) = args.wikipedia_urls {
        info!("Loading article urls from {path:?}");
        parse_wikipedia_file(path)?
    } else {
        Default::default()
    };

    let mut wikidata_ids = if let Some(path) = args.wikidata_ids {
        info!("Loading wikidata ids from {path:?}");
        parse_wikidata_file(path)?
    } else {
        Default::default()
    };

    if let Some(path) = args.osm_tags {
        info!("Loading wikipedia/wikidata osm tags from {path:?}");
        parse_osm_tag_file(path, &mut wikidata_ids, &mut wikipedia_titles)?;
    }

    debug!("Parsed {} unique article urls", wikipedia_titles.len());
    debug!("Parsed {} unique wikidata ids", wikidata_ids.len());

    // NOTE: For atomic writes to the same file across threads/processes:
    // - The file needs to be opened in APPEND mode (`.append(true)`).
    // - Each write needs to be a single syscall (for Rust, use `format!` for formatting before calling `write!`, or `write!` to a `String` first).
    // - Each write needs to be under `PIPE_BUF` size (see `man write(3)`), usually 4kb on Linux.
    //
    // For more information, see:
    // - `man write(3posix)`: https://www.man7.org/linux/man-pages/man3/write.3p.html
    // - `std::fs::OpenOptions::append`: https://doc.rust-lang.org/std/fs/struct.OpenOptions.html#method.append
    // - https://stackoverflow.com/questions/1154446/is-file-append-atomic-in-unix
    let mut write_new_ids = args
        .write_new_ids
        .as_ref()
        .map(|p| File::options().create(true).append(true).open(p))
        .transpose()?;

    if !args.output_dir.is_dir() {
        bail!("output dir {:?} does not exist", args.output_dir)
    }

    info!("Processing dump");
    let dump = stdin().lock();

    // TODO: Compare different deserialization methods.
    // The docs warn against using a reader directly, and it's slower than tar can decompress the dump.
    // let stream = serde_json::Deserializer::from_reader(dump).into_iter::<Page>();
    let stream = dump.lines().map(|r| {
        r.map_err(anyhow::Error::new)
            .and_then(|s| serde_json::from_str::<Page>(&s).map_err(anyhow::Error::new))
    });

    for page in stream {
        let page = page?;

        let qid = page.wikidata();

        let is_wikidata_match = qid
            .as_ref()
            .map(|qid| wikidata_ids.contains(qid))
            .unwrap_or_default();

        let matching_titles = if wikipedia_titles.is_empty() {
            Default::default()
        } else {
            page.all_titles()
                .filter_map(|r| {
                    r.map(Some).unwrap_or_else(|e| {
                        warn!("Could not parse title for {:?}: {:#}", &page.name, e);
                        None
                    })
                })
                .filter(|t| wikipedia_titles.contains(t))
                .collect::<Vec<_>>()
        };

        if !is_wikidata_match && matching_titles.is_empty() {
            continue;
        }

        // Write matched new QIDs back to fild.
        if let (Some(f), Some(qid)) = (&mut write_new_ids, &qid) {
            if !is_wikidata_match && !matching_titles.is_empty() {
                debug!("Writing new id {} for article {:?}", qid, page.name);
                // NOTE: Write to string buffer first to have a single atomic write syscall.
                // See `write_new_ids` for more info.
                let line = format!("{}\n", qid);
                write!(f, "{}", line).with_context(|| {
                    format!(
                        "writing new id to file {:?}",
                        args.write_new_ids.as_ref().unwrap()
                    )
                })?;
            }
        }

        if let Err(e) = write(&args.output_dir, &page, matching_titles) {
            error!("Error writing article {:?}: {:#}", page.name, e);
        }
    }

    Ok(())
}
