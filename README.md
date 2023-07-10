# wikiparser

_Extracts articles from [Wikipedia database dumps](https://en.wikipedia.org/wiki/Wikipedia:Database_download) for embedding into the `mwm` map files created by [the Organic Maps generator](https://github.com/organicmaps/organicmaps/blob/master/tools/python/maps_generator/README.md)._

## Configuring

[`article_processing_config.json`](article_processing_config.json) should be updated when adding a new language.
It defines article sections that are not important for users and should be removed from the extracted HTML.

## Usage

First, install [the rust language tools](https://www.rust-lang.org/)

For best performance, use `--release` when building or running.

You can run the program from within this directory using `cargo run --release --`.

Alternatively, build it with `cargo build --release`, which places the binary in `./target/release/om-wikiparser`.

Run the program with the `--help` flag to see all supported arguments.

It takes as inputs:
- A wikidata enterprise JSON dump, extracted and connected to `stdin`.
- A file of Wikidata QIDs to extract, one per line (e.g. `Q12345`), passed as the CLI flag `--wikidata-ids`.
- A file of Wikipedia article titles to extract, one per line (e.g. `https://$LANG.wikipedia.org/wiki/$ARTICLE_TITLE`), passed as a CLI flag `--wikipedia-urls`.
- A directory to write the extracted articles to, as a CLI argument.

As an example of usage with the map generator:
- Assuming this program is installed to `$PATH` as `om-wikiparser`.
- Download [the dumps in the desired languages](https://dumps.wikimedia.org/other/enterprise_html/runs/) (Use the files with the format `${LANG}wiki-NS0-${DATE}-ENTERPRISE-HTML.json.tar.gz`).
  Set `DUMP_DOWNLOAD_DIR` to the location they are downloaded.
- Run the following from within the `intermediate_data` subdirectory of the maps build directory:

```shell
# Transform intermediate files from generator.
cut -f 2 id_to_wikidata.csv > wikidata_ids.txt
tail -n +2 wiki_urls.txt | cut -f 3 > wikipedia_urls.txt
# Begin extraction.
for dump in $DUMP_DOWNLOAD_DIR/*-ENTERPRISE-HTML.json.tar.gz
do
  tar xzf $dump | om-wikiparser \
    --wikidata-ids wikidata_ids.txt \
    --wikipedia-urls wikipedia_urls.txt \
    descriptions/
done
```
