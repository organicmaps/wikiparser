use std::{iter, str::FromStr};

use serde::Deserialize;

use super::{WikidataQid, WikipediaTitleNorm};

// TODO: consolidate into single struct
/// Deserialized Wikimedia Enterprise API Article
///
/// For all available fields, see <https://enterprise.wikimedia.com/docs/data-dictionary/>.
#[allow(dead_code)] // TODO: reevaluate fields
#[derive(Deserialize)]
pub struct Page {
    // TODO: Check if CoW has a performance impact.
    pub name: String,
    pub date_modified: String,
    pub in_language: Language,
    #[serde(default)]
    pub url: String,
    pub main_entity: Option<Wikidata>,
    // TODO: See what impact parsing/unescaping/allocating this has.
    pub article_body: ArticleBody,
    #[serde(default)]
    pub redirects: Vec<Redirect>,
}

impl Page {
    pub fn wikidata(&self) -> Option<WikidataQid> {
        // TODO: return error
        self.main_entity
            .as_ref()
            .map(|e| WikidataQid::from_str(&e.identifier).unwrap())
    }

    /// Title of the article
    pub fn title(&self) -> anyhow::Result<WikipediaTitleNorm> {
        WikipediaTitleNorm::from_title(&self.name, &self.in_language.identifier)
    }

    /// All titles that lead to the article, the main title followed by any redirects.
    pub fn all_titles(&self) -> impl Iterator<Item = anyhow::Result<WikipediaTitleNorm>> + '_ {
        iter::once(self.title()).chain(self.redirects())
    }

    pub fn redirects(&self) -> impl Iterator<Item = anyhow::Result<WikipediaTitleNorm>> + '_ {
        self.redirects
            .iter()
            .map(|r| WikipediaTitleNorm::from_title(&r.name, &self.in_language.identifier))
    }
}

#[derive(Deserialize)]
pub struct Wikidata {
    pub identifier: String,
}

#[derive(Deserialize)]
pub struct ArticleBody {
    // TODO: Look into RawValue to lazily parse/allocate this:
    // https://docs.rs/serde_json/latest/serde_json/value/struct.RawValue.html
    pub html: String,
}

#[allow(dead_code)] // TODO: Reevaluate fields.
#[derive(Deserialize)]
pub struct Redirect {
    pub url: String,
    pub name: String,
}

#[derive(Deserialize)]
pub struct Language {
    pub identifier: String,
}
