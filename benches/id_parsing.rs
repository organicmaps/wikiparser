#![feature(test)]
use std::{collections::HashSet, str::FromStr};

extern crate om_wikiparser;
extern crate test;

#[bench]
fn parse_wikipedia(b: &mut test::Bencher) {
    b.iter(|| {
        let title = om_wikiparser::wm::WikipediaTitleNorm::from_url(
            "https://en.wikipedia.org/wiki/Article_Title",
        )
        .unwrap();
    });
}

#[bench]
fn hash_wikipedia(b: &mut test::Bencher) {
    let title = om_wikiparser::wm::WikipediaTitleNorm::from_url(
        "https://en.wikipedia.org/wiki/Article_Title",
    )
    .unwrap();
    let mut set = HashSet::new();
    b.iter(|| {
        set.insert(&title);
    });
}

#[bench]
fn parse_wikidata(b: &mut test::Bencher) {
    b.iter(|| {
        let qid = om_wikiparser::wm::WikidataQid::from_str("Q123456789").unwrap();
    });
}

#[bench]
fn hash_wikidata(b: &mut test::Bencher) {
    let qid = om_wikiparser::wm::WikidataQid::from_str("Q123456789").unwrap();
    let mut set = HashSet::new();
    b.iter(|| {
        set.insert(&qid);
    });
}
