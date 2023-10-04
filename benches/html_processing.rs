#![feature(test)]

extern crate om_wikiparser;
extern crate test;

use test::{bench::black_box, Bencher};

use om_wikiparser::html;

#[bench]
fn process_crimean_mountains(b: &mut Bencher) {
    let text = include_str!("../tests/data/Q4185820-en/original.html");

    // process lazy statics beforehand
    black_box(html::process_str(text, "en").unwrap());

    b.iter(|| {
        black_box(html::process_str(text, "en").unwrap());
    });
}
