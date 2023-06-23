use serde::Deserialize;

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
