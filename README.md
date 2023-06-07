# wikiparser

_Extracts articles from [Wikipedia database dumps](https://en.wikipedia.org/wiki/Wikipedia:Database_download) for embedding into the `mwm` map files created by [the Organic Maps generator](https://github.com/organicmaps/organicmaps/blob/master/tools/python/maps_generator/README.md)._

## Usage

[`article_processing_config.json`](article_processing_config.json) should be updated when adding a new language.
It defines article sections that are not important for users and should be removed.
