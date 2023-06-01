// Usage:
//     # prep outputs from map generator
//     cut -f 2 ~/Downloads/id_to_wikidata.csv > /tmp/wikidata_ids.txt
//     tail -n +2 ~/Downloads/wiki_urls.txt | cut -f 3 > /tmp/wikipedia_urls.txt
//     # feed gzipped tarfile
//     pv ~/Downloads/enwiki-NS0-20230401-ENTERPRISE-HTML.json.tar.gz | tar xzO \
//     | cargo run --release -- \
//     --wikidata-ids /tmp/wikidata_ids.txt \
//     --wikipedia-urls /tmp/wikipedia_urls.txt \
//     output_dir
use std::{
    fs::File,
    io::{stdin, BufRead, BufReader, Write},
    path::PathBuf,
};

use anyhow::bail;
use clap::Parser;
#[macro_use]
extern crate log;

use om_wikiparser::{
    html::simplify,
    wm::{is_wikidata_match, is_wikipedia_match, parse_wikidata_file, parse_wikipedia_file, Page},
};

#[derive(Parser)]
struct Args {
    output_dir: PathBuf,
    #[arg(long)]
    wikidata_ids: Option<PathBuf>,
    #[arg(long)]
    wikipedia_urls: Option<PathBuf>,
}

fn main() -> anyhow::Result<()> {
    env_logger::Builder::new()
        .filter_level(log::LevelFilter::Info)
        .parse_default_env()
        .try_init()?;

    let args = Args::parse();

    info!("Loading urls");
    let wikipedia_titles = args
        .wikipedia_urls
        .map(parse_wikipedia_file)
        .transpose()?
        .unwrap_or_default();

    info!("Loading ids");
    let wikidata_ids = args
        .wikidata_ids
        .map(parse_wikidata_file)
        .transpose()?
        .unwrap_or_default();

    if !args.output_dir.is_dir() {
        bail!("output dir {:?} does not exist", args.output_dir)
    }

    info!("Processing dump");
    let dump = BufReader::new(stdin());

    // TODO: compare different deserialization methods
    // docs warn against using a reader directly, and it's slower than tar can decompress the dump
    // let stream = serde_json::Deserializer::from_reader(dump).into_iter::<Page>();
    let stream = dump.lines().map(|r| {
        r.map_err(anyhow::Error::new)
            .and_then(|s| serde_json::from_str::<Page>(&s).map_err(anyhow::Error::new))
    });

    for page in stream {
        let page = page?;

        if !(is_wikidata_match(&wikidata_ids, &page).is_some()
            || is_wikipedia_match(&wikipedia_titles, &page).is_some())
        {
            continue;
        }

        let Some(qid) = page.main_entity.map(|e| e.identifier) else {
            warn!("Page in list but without wikidata qid: {:?}", page.name);
            continue;
        };

        let filename = args.output_dir.join(qid).with_extension("html");

        debug!("{:?}: {:?}", page.name, filename);

        if filename.exists() {
            debug!("Exists, skipping");
            continue;
        }

        let html = simplify(&page.article_body.html);

        let mut file = File::create(filename)?;
        file.write_all(html.as_bytes())?;
    }

    Ok(())
}
