# luoxu-rs

> **WARNING: WORK IN PROGRESS**, we welcome testing and improvements before production deployment.

Matrix Chatgroup index and searching backed by [Meilisearch](https://www.meilisearch.com).

## Setup

Copy the provided [`luoxu-rs.sample.toml`](luoxu-rs.sample.toml) to `luoxu-rs.toml` and edit paramters.

## Run

If running from source:

```console
$ cargo run --bin luoxu-rs # For the bot
$ cargo run --bin luoxu-rs-web # For the Web API
```