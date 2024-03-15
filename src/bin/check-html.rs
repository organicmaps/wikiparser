use std::{
    fs,
    io::{self, stdin, stdout},
    path::{Path, PathBuf},
};

use clap::Parser;
use scraper::{selector::ToCss, Html, Selector};

use om_wikiparser::html::{self, HtmlError};

#[derive(Parser)]
struct Args {
    #[clap(short, long)]
    selectors: Vec<String>,
}

#[derive(Debug)]
struct Stats {
    file: PathBuf,
    lang: Option<String>,
    title: Option<String>,
    original_size: usize,
    processed_size: Option<usize>,
    error: Option<HtmlError>,
    redirect: Option<String>,
}

fn check(path: impl AsRef<Path>, selectors: &[Selector]) -> io::Result<(Stats, Vec<usize>)> {
    let file = path.as_ref().to_owned();
    let contents = fs::read_to_string(&file)?;
    let original_size = contents.len();
    let html = Html::parse_document(&contents);
    let title = html::get_title(&html).map(str::to_string);

    let lang = html::detect_lang(&html);
    let redirect = html::detect_redirect(&html).map(str::to_string);

    let selectors = selectors.iter().map(|s| html.select(s).count()).collect();

    let (processed_size, error) = html::process(html, lang.as_deref().unwrap_or("en"))
        .map_or_else(|e| (None, Some(e)), |html| (Some(html.html().len()), None));

    Ok((
        Stats {
            file,
            lang,
            title,
            original_size,
            processed_size,
            error,
            redirect,
        },
        selectors,
    ))
}

fn write_header<W: io::Write>(
    wtr: &mut csv::Writer<W>,
    selectors: impl IntoIterator<Item = String>,
) -> csv::Result<()> {
    for header in [
        "file",
        "lang",
        "title",
        "original_size",
        "processed_size",
        "error",
        "redirect",
    ] {
        wtr.write_field(header)?;
    }
    for selector in selectors {
        wtr.write_field(selector)?;
    }
    wtr.write_record::<&[String; 0], _>(&[])?;
    Ok(())
}

fn write_fields<W: io::Write>(
    wtr: &mut csv::Writer<W>,
    stats: Stats,
    selector_counts: &[usize],
) -> csv::Result<()> {
    let Stats {
        file,
        lang,
        title,
        original_size,
        processed_size,
        error,
        redirect,
    } = stats;

    wtr.write_record(
        [
            file.to_string_lossy().to_string(),
            lang.unwrap_or_default(),
            title.unwrap_or_default(),
            original_size.to_string(),
            processed_size.map(|s| s.to_string()).unwrap_or_default(),
            error.map(|e| format!("{:?}", e)).unwrap_or_default(),
            redirect.unwrap_or_default(),
        ]
        .into_iter()
        .chain(selector_counts.iter().map(ToString::to_string)),
    )
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let selectors: Vec<_> = args
        .selectors
        .iter()
        .map(|s| Selector::parse(s).unwrap())
        .collect();

    let mut wtr = csv::WriterBuilder::new()
        .delimiter(b'\t')
        .from_writer(stdout());

    write_header(&mut wtr, selectors.iter().map(|s| s.to_css_string()))?;

    for line in stdin().lines() {
        let (stats, selectors) = check(line?, &selectors)?;
        write_fields(&mut wtr, stats, &selectors)?;
    }

    Ok(())
}
