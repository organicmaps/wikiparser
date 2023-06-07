//! Apply html article simplification to stdin, and write it to stdout.
//!
//! Usage:
//!     simplify_html < article.html > simplified.html
use std::io::{stdin, stdout, Read, Write};

use om_wikiparser::html::simplify;

fn main() -> anyhow::Result<()> {
    let mut input = String::new();
    stdin().read_to_string(&mut input)?;

    let output = simplify(&input, "en");

    stdout().write_all(output.as_bytes())?;

    Ok(())
}
