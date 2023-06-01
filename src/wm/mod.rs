//! Wikimedia types
use std::{
    collections::HashSet,
    ffi::OsStr,
    fs::{self},
    num::ParseIntError,
    str::FromStr,
};

use anyhow::{anyhow, bail, Context};

use url::Url;

mod page;
pub use page::Page;

/// Read from a file of urls on each line.
pub fn parse_wikidata_file(path: impl AsRef<OsStr>) -> anyhow::Result<HashSet<WikidataQid>> {
    let contents = fs::read_to_string(path.as_ref())?;
    contents
        .lines()
        .enumerate()
        .map(|(i, line)| {
            WikidataQid::from_str(line).with_context(|| {
                let line_num = i + 1;
                format!("bad QID value on line {line_num}: {line:?}")
            })
        })
        .collect()
}

/// Read article titles from a file of urls on each line.
pub fn parse_wikipedia_file(
    path: impl AsRef<OsStr>,
) -> anyhow::Result<HashSet<WikipediaTitleNorm>> {
    let contents = fs::read_to_string(path.as_ref())?;
    contents
        .lines()
        .enumerate()
        .map(|(i, line)| {
            WikipediaTitleNorm::from_url(line).with_context(|| {
                let line_num = i + 1;
                format!("bad wikipedia url on line {line_num}: {line:?}")
            })
        })
        .collect()
}

pub fn is_wikidata_match(ids: &HashSet<WikidataQid>, page: &Page) -> Option<WikidataQid> {
    let Some(wikidata) = &page.main_entity else { return None;};
    let wikidata_id = &wikidata.identifier;
    let wikidata_id = match WikidataQid::from_str(wikidata_id) {
        Ok(qid) => qid,
        Err(e) => {
            eprintln!("Could not parse QID: {:?}: {}", wikidata_id, e);
            return None;
        }
    };

    ids.get(&wikidata_id).map(|_| wikidata_id)
}

pub fn is_wikipedia_match(
    titles: &HashSet<WikipediaTitleNorm>,
    page: &Page,
) -> Option<WikipediaTitleNorm> {
    // TODO: handle multiple languages
    let title = WikipediaTitleNorm::from_title(&page.name, "en");

    if titles.get(&title).is_some() {
        return Some(title);
    }

    for redirect in &page.redirects {
        let title = WikipediaTitleNorm::from_title(&redirect.name, "en");

        if titles.get(&title).is_some() {
            return Some(title);
        }
    }

    None
}

/// Wikidata QID/Q Number
///
/// See https://www.wikidata.org/wiki/Wikidata:Glossary#QID
///
/// ```
/// use std::str::FromStr;
/// use om_wikiparser::wm::WikidataQid;
///
/// let with_q = WikidataQid::from_str("Q12345").unwrap();
/// let without_q = WikidataQid::from_str("12345").unwrap();
/// assert_eq!(with_q, without_q);
/// ```
#[derive(Debug, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub struct WikidataQid(u32);

impl FromStr for WikidataQid {
    type Err = ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.strip_prefix('Q').unwrap_or(s);
        u32::from_str(s).map(WikidataQid)
    }
}

/// Normalized wikipedia article title that can compare:
/// - titles `Spatial Database`
/// - urls `https://en.wikipedia.org/wiki/Spatial_database#Geodatabase`
/// - osm-style tags `en:Spatial Database`
///
/// ```
/// use om_wikiparser::wm::WikipediaTitleNorm;
///
/// let url = WikipediaTitleNorm::from_url("https://en.wikipedia.org/wiki/Article_Title/").unwrap();
/// let title = WikipediaTitleNorm::from_title("Article Title", "en");
/// assert_eq!(url, title);
/// ```
#[derive(Debug, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub struct WikipediaTitleNorm {
    lang: String,
    name: String,
}

impl WikipediaTitleNorm {
    fn normalize_title(title: &str) -> String {
        // TODO: compare with generator url creation
        title.replace(' ', "_")
    }

    // https://en.wikipedia.org/wiki/Article_Title
    pub fn from_url(url: &str) -> anyhow::Result<Self> {
        let url = Url::parse(url)?;

        let (subdomain, host) = url
            .host_str()
            .ok_or(anyhow!("Expected host"))?
            .split_once('.')
            .ok_or(anyhow!("Expected subdomain"))?;
        if host != "wikipedia.org" {
            bail!("Expected wikipedia.org for domain")
        }
        let lang = subdomain;

        let mut paths = url.path_segments().ok_or(anyhow!("Expected path"))?;

        let root = paths
            .next()
            .ok_or(anyhow!("Expected first segment in path"))?;

        if root != "wiki" {
            bail!("Expected 'wiki' in path")
        }

        let title = paths
            .next()
            .ok_or(anyhow!("Expected second segment in path"))?;
        let title = urlencoding::decode(title)?;

        Ok(Self::from_title(&title, lang))
    }

    // en:Article Title
    fn _from_osm_tag(tag: &str) -> anyhow::Result<Self> {
        let (lang, title) = tag.split_once(':').ok_or(anyhow!("Expected ':'"))?;

        Ok(Self::from_title(title, lang))
    }

    pub fn from_title(title: &str, lang: &str) -> Self {
        let name = Self::normalize_title(title);
        let lang = lang.to_owned();
        Self { name, lang }
    }
}
