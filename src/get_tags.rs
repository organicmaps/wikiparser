use std::{
    io::{stdout, Read},
    sync::mpsc,
    thread,
};

use om_wikiparser::osm::{Id, Kind, Version};
use osmpbf::{BlobDecode, BlobReader, Element};
use rayon::prelude::*;

struct Record {
    id: Id,
    kind: Kind,
    version: Option<Version>,
    wikidata: String,
    wikipedia: String,
}

/// Extract matching tags from an osm pbf file and write to stdout in TSV.
pub fn run(pbf: impl Read + Send) -> anyhow::Result<()> {
    let reader = BlobReader::new(pbf);

    let (send, recv) = mpsc::sync_channel(128);
    let writer_thread = thread::Builder::new()
        .name("writer".to_string())
        .spawn(move || write(recv))?;

    reader
        .par_bridge()
        .try_for_each(move |blob| -> anyhow::Result<()> {
            // Based on `osmpbf` implementation of `ElementReader`.
            let BlobDecode::OsmData(block) = blob?.decode()? else {
                return Ok(());
            };
            for record in block.elements().filter_map(extract_tags) {
                send.send(record)?;
            }
            Ok(())
        })?;

    let record_count = writer_thread.join().unwrap()?;
    info!("Finished processing {record_count} records");

    Ok(())
}

fn write(recv: mpsc::Receiver<Record>) -> anyhow::Result<usize> {
    let mut output = csv::WriterBuilder::new()
        .delimiter(b'\t')
        .from_writer(stdout().lock());
    output.write_record(["@id", "@otype", "@version", "wikidata", "wikipedia"])?;

    let mut count = 0;

    for Record {
        id,
        kind,
        version,
        wikidata,
        wikipedia,
    } in recv
    {
        output.write_record([
            id.to_string(),
            kind.otype().to_string(),
            version.map(|v| v.to_string()).unwrap_or_default(),
            wikidata,
            wikipedia,
        ])?;
        count += 1;
    }

    Ok(count)
}

#[rustfmt::skip]
fn extract_tags(el: Element) -> Option<Record> {
    match el {
        Element::Node(n) =>      make_record(Kind::Node,     n.id(), n.info().version(),            n.tags()),
        Element::DenseNode(n) => make_record(Kind::Node,     n.id(), n.info().map(|i| i.version()), n.tags()),
        Element::Way(w) =>       make_record(Kind::Way,      w.id(), w.info().version(),            w.tags()),
        Element::Relation(r) =>  make_record(Kind::Relation, r.id(), r.info().version(),            r.tags()),
    }
}

fn make_record<'i>(
    kind: Kind,
    id: Id,
    version: Option<Version>,
    tags: impl 'i + Iterator<Item = (&'i str, &'i str)>,
) -> Option<Record> {
    let mut wikipedia = String::new();
    let mut wikidata = String::new();

    for (key, value) in tags {
        match key {
            "wikipedia" => wikipedia = value.trim().to_owned(),
            "wikidata" => wikidata = value.trim().to_owned(),
            _ => {}
        }
    }

    if wikidata.is_empty() && wikipedia.is_empty() {
        return None;
    }

    Some(Record {
        id,
        kind,
        version,
        wikipedia,
        wikidata,
    })
}
