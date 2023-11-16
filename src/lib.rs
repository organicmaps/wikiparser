use std::{collections::HashSet, ffi::OsStr, fs, str::FromStr};

#[macro_use]
extern crate log;
use anyhow::Context;

pub mod html;
pub mod osm;
mod tag_file;
pub use tag_file::*;
pub mod wm;

use wm::{Qid, Title};

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
            Title::from_osm_tag(line).with_context(|| {
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
