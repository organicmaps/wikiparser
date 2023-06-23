use std::collections::{BTreeMap, BTreeSet};

use once_cell::sync::Lazy;
use scraper::{ElementRef, Html, Selector};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Config<'a> {
    #[serde(borrow)]
    sections_to_remove: BTreeMap<&'a str, BTreeSet<&'a str>>,
}

static CONFIG: Lazy<Config<'static>> = Lazy::new(|| {
    serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/article_processing_config.json"
    )))
    .expect("\"article_processing_config.json\" is either invalid json or the wrong structure")
});

static HEADERS: Lazy<Selector> =
    Lazy::new(|| Selector::parse("h1, h2, h3, h4, h5, h6, h7").unwrap());

pub fn simplify(html: &str, lang: &str) -> String {
    let mut document = Html::parse_document(html);

    let mut to_remove = Vec::new();

    // Remove configured sections and all trailing elements until next section.

    if let Some(bad_sections) = CONFIG.sections_to_remove.get(lang) {
        for header in document.select(&HEADERS) {
            // TODO: Should this join all text nodes?
            let Some(title) = header.text().next() else {
                continue
            };

            if bad_sections.contains(&title.trim()) {
                to_remove.push(header.id());
                let header_level = header.value().name();
                // Strip trailing nodes.
                for sibling in header.next_siblings() {
                    if let Some(element) = sibling.value().as_element() {
                        if element.name() == header_level {
                            // TODO: Should this check for a higher level?
                            break;
                        }
                    }
                    to_remove.push(sibling.id());
                }
            }
        }

        for id in to_remove.drain(..) {
            if let Some(mut node) = document.tree.get_mut(id) {
                node.detach();
            }
        }
    } else {
        warn!("No sections to remove configured for lang {lang:?}");
    }

    // Remove elements with no text that isn't whitespace.

    for element in document
        .root_element()
        .descendants()
        .filter_map(ElementRef::wrap)
    {
        if element.text().all(|t| t.trim().is_empty()) {
            to_remove.push(element.id());
        }
    }

    for id in to_remove.drain(..) {
        if let Some(mut node) = document.tree.get_mut(id) {
            node.detach();
        }
    }

    document.html()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn static_config_parses() {
        assert!(!CONFIG.sections_to_remove.is_empty());
    }
}
