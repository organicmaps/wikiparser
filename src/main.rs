// Usage:
//     pv ~/Downloads/enwiki-NS0-20230401-ENTERPRISE-HTML.json.tar.gz | tar xzO | cargo run --release > /dev/null

use serde::Deserialize;
use std::io::{self, stdin, BufRead, BufReader, Write};

#[derive(Deserialize)]
struct Page {
    // TODO: check if CoW has a performance impact
    name: String,
    date_modified: String,
    #[serde(default)]
    url: String,
    main_entity: Option<Wikidata>,
    // TODO: see what impact parsing/unescaping/allocating this has
    article_body: ArticleBody,
    #[serde(default)]
    redirects: Vec<Redirect>,
}

#[derive(Deserialize)]
struct Wikidata {
    identifier: String,
}

#[derive(Deserialize)]
struct ArticleBody {
    html: String,
}

#[derive(Deserialize)]
struct Redirect {
    url: String,
    name: String,
}

fn main() -> anyhow::Result<()> {
    let dump = BufReader::new(stdin());

    // TODO: compare different deserialization methods
    // docs warn against using a reader directly, and it's slower than tar can decompress the dump
    // let stream = serde_json::Deserializer::from_reader(dump).into_iter::<Page>();
    let stream = dump.lines().map(|r| {
        r.map_err(anyhow::Error::new)
            .and_then(|s| serde_json::from_str::<Page>(&s).map_err(anyhow::Error::new))
    });

    let mut stdout = io::stdout();
    for page in stream {
        let page = page?;
        writeln!(stdout, "{}", page.name)?;
    }

    Ok(())
}
