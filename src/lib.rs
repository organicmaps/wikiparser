use std::{
    io::{self, BufRead},
    str::FromStr,
};

#[macro_use]
extern crate log;

pub mod html;
pub mod osm;
mod tag_file;
pub use tag_file::*;
pub mod extend;
pub mod wm;

use wm::{Qid, Title};

/// Read from a file of urls on each line.
pub fn parse_wikidata_file(r: impl BufRead, collection: &mut impl Extend<Qid>) -> io::Result<()> {
    for (i, line) in r.lines().enumerate() {
        let line = line?;
        match Qid::from_str(&line) {
            Ok(qid) => collection.extend(Some(qid)),
            Err(e) => {
                let line_num = i + 1;
                warn!("Could not parse QID: on line {line_num}: {line:?}: {:#}", e);
            }
        }
    }
    Ok(())
}

/// Read article titles from a file of urls on each line.
pub fn parse_wikipedia_file(
    r: impl BufRead,
    collection: &mut impl Extend<Title>,
) -> io::Result<()> {
    for (i, line) in r.lines().enumerate() {
        let line = line?;
        match Title::from_osm_tag(&line) {
            Ok(title) => collection.extend(Some(title)),
            Err(e) => {
                let line_num = i + 1;
                warn!(
                    "Could not parse wikipedia title: on line {line_num}: {line:?}: {:#}",
                    e
                );
            }
        }
    }
    Ok(())
}
