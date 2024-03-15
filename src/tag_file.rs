use std::{error::Error, fmt::Display, io::Read, str::FromStr};

use anyhow::{anyhow, bail};

use crate::{
    osm,
    wm::{ParseQidError, ParseTitleError, Qid, Title},
};

/// Read a TSV file of OSM tags, using wikipedia/wikidata tags.
pub fn parse_osm_tag_file(
    r: impl Read,
    qids: &mut impl Extend<Qid>,
    titles: &mut impl Extend<Title>,
    line_errors: &mut impl Extend<ParseLineError>,
) -> anyhow::Result<()> {
    let mut rdr = csv::ReaderBuilder::new().delimiter(b'\t').from_reader(r);

    let mut push_error = |e: ParseLineError| {
        line_errors.extend(Some(e));
    };

    let mut qid_col = None;
    let mut title_col = None;
    let mut osm_id_col = None;
    let mut osm_otype_col = None;
    let mut osm_oname_col = None;
    let mut osm_version_col = None;
    for (column, title) in rdr.headers()?.iter().enumerate() {
        match title {
            "wikidata" => qid_col = Some(column),
            "wikipedia" => title_col = Some(column),
            "@id" => osm_id_col = Some(column),
            "@otype" => osm_otype_col = Some(column),
            "@oname" => osm_oname_col = Some(column),
            "@version" => osm_version_col = Some(column),
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
                if e.is_io_error() {
                    bail!(e)
                }
                push_error(ParseLineError {
                    kind: e.into(),
                    text: String::new(),
                    line: rdr.position().line(),
                    osm_id: None,
                    osm_type: None,
                    osm_version: None,
                });
                continue;
            }
        }

        let parse_metadata = || {
            (
                osm_id_col.and_then(|i| row[i].trim().parse::<osm::Id>().ok()),
                // Prefer otype, use oname if not available
                osm_otype_col
                    .and_then(|i| row[i].trim().parse().ok())
                    .and_then(osm::Kind::from_otype)
                    .or_else(|| osm_oname_col.and_then(|i| osm::Kind::from_oname(&row[i]))),
                osm_version_col.and_then(|i| row[i].trim().parse::<osm::Version>().ok()),
            )
        };

        let qid = &row[qid_col].trim();
        if !qid.is_empty() {
            match Qid::from_str(qid) {
                Ok(qid) => {
                    qids.extend(Some(qid));
                }
                Err(e) => {
                    let (osm_id, osm_type, osm_version) = parse_metadata();
                    push_error(ParseLineError {
                        kind: e.into(),
                        text: qid.to_string(),
                        line: rdr.position().line(),
                        osm_id,
                        osm_type,
                        osm_version,
                    })
                }
            }
        }

        let title = &row[title_col].trim();
        if !title.is_empty() {
            match Title::from_osm_tag(title) {
                Ok(title) => {
                    titles.extend(Some(title));
                }
                Err(e) => {
                    let (osm_id, osm_type, osm_version) = parse_metadata();
                    push_error(ParseLineError {
                        kind: e.into(),
                        text: title.to_string(),
                        line: rdr.position().line(),
                        osm_id,
                        osm_type,
                        osm_version,
                    })
                }
            }
        }
    }

    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum ParseErrorKind {
    #[error("title")]
    Title(#[from] ParseTitleError),
    #[error("QID")]
    Qid(#[from] ParseQidError),
    #[error("TSV line")]
    Tsv(#[from] csv::Error),
}

#[derive(Debug)]
pub struct ParseLineError {
    pub kind: ParseErrorKind,
    pub text: String,
    pub line: u64,
    pub osm_id: Option<osm::Id>,
    pub osm_type: Option<osm::Kind>,
    pub osm_version: Option<osm::Version>,
}

impl Display for ParseLineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "on line {}", self.line)?;
        if let Some(osm_id) = self.osm_id {
            write!(f, " ({osm_id})")?;
        }
        write!(f, ": {} {:?}", self.kind, self.text)?;

        // Write source error chain to ensure they are logged.
        let mut source = self.kind.source();
        while let Some(e) = source {
            write!(f, ": {}", e)?;
            source = e.source();
        }
        Ok(())
    }
}

impl Error for ParseLineError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        // Return nothing because Display prints source chain.
        None
    }
}
