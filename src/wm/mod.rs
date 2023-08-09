//! Wikimedia types
use std::{collections::HashSet, ffi::OsStr, fs, str::FromStr};

use anyhow::{anyhow, Context};

mod page;
pub use page::Page;
mod title;
pub use title::*;
mod qid;
pub use qid::*;

/// Read from a file of urls on each line.
pub fn parse_wikidata_file(path: impl AsRef<OsStr>) -> anyhow::Result<HashSet<Qid>> {
    let contents = fs::read_to_string(path.as_ref())?;
    Ok(contents
        .lines()
        .enumerate()
        .map(|(i, line)| {
            Qid::from_str(line).with_context(|| {
                let line_num = i + 1;
                format!("on line {line_num}: {line:?}")
            })
        })
        .filter_map(|r| match r {
            Ok(qid) => Some(qid),
            Err(e) => {
                warn!("Could not parse QID: {:#}", e);
                None
            }
        })
        .collect())
}

/// Read article titles from a file of urls on each line.
pub fn parse_wikipedia_file(path: impl AsRef<OsStr>) -> anyhow::Result<HashSet<Title>> {
    let contents = fs::read_to_string(path.as_ref())?;
    Ok(contents
        .lines()
        .enumerate()
        .map(|(i, line)| {
            Title::from_url(line).with_context(|| {
                let line_num = i + 1;
                format!("on line {line_num}: {line:?}")
            })
        })
        .filter_map(|r| match r {
            Ok(qid) => Some(qid),
            Err(e) => {
                warn!("Could not parse wikipedia title: {:#}", e);
                None
            }
        })
        .collect())
}

pub fn parse_osm_tag_file(
    path: impl AsRef<OsStr>,
    qids: &mut HashSet<Qid>,
    titles: &mut HashSet<Title>,
) -> anyhow::Result<()> {
    let path = path.as_ref();
    let mut rdr = csv::ReaderBuilder::new().delimiter(b'\t').from_path(path)?;

    let mut qid_col = None;
    let mut title_col = None;
    for (column, title) in rdr.headers()?.iter().enumerate() {
        match title {
            "wikidata" => qid_col = Some(column),
            "wikipedia" => title_col = Some(column),
            _ => (),
        }
    }

    let qid_col = qid_col.ok_or_else(|| anyhow!("Cannot find 'wikidata' column"))?;
    let title_col = title_col.ok_or_else(|| anyhow!("Cannot find 'wikipedia' column"))?;

    let mut row = csv::StringRecord::new();
    loop {
        match rdr.read_record(&mut row) {
            Ok(true) => {}
            // finished
            Ok(false) => break,
            // attempt to recover from parsing errors
            Err(e) => {
                error!("Error parsing tsv file: {}", e);
                continue;
            }
        }

        let qid = &row[qid_col].trim();
        if !qid.is_empty() {
            match Qid::from_str(qid) {
                Ok(qid) => {
                    qids.insert(qid);
                }
                Err(e) => warn!(
                    "Cannot parse qid {:?} on line {} in {:?}: {}",
                    qid,
                    rdr.position().line(),
                    path,
                    e
                ),
            }
        }

        let title = &row[title_col].trim();
        if !title.is_empty() {
            match Title::from_osm_tag(title) {
                Ok(title) => {
                    titles.insert(title);
                }
                Err(e) => warn!(
                    "Cannot parse title {:?} on line {} in {:?}: {}",
                    title,
                    rdr.position().line(),
                    path,
                    e
                ),
            }
        }
    }

    Ok(())
}
