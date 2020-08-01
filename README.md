# Tempoit

Simple timewarrior to tempo/jira worklog uploader.

Copyright © 2020 Samuel Walladge

## About

This is a small tool I use for my work time logging workflow.
My work tracks time using Tempo (a Jira plugin).
I track time locally using timewarrior, using tags to mark logs for work that require logging to tempo.
This tool filters and parses the timewarrior logs, converts them to tempo-compatible worklogs,
uploads them, and marks them as uploaded in timewarrior by modifying the tags.

## Usage

Install from crates: `cargo install tempoit`, or clone this repository and `cargo run` or `cargo build`.
You should have a recent stable rust toolchain installed.

On first run, `~/.config/tempoit/tempoit.toml` (or equivalent default path for your system) will be created.

Configure to suit - example below:

```
# your jira credentials
username = 'my_username'
password = 'my_password'
# base url of the jira instance
base_url = 'https://tasks.opencraft.com'
```

Then run `tempoit`. For example:

```
❯ tempoit
:: Ready to upload worklogs:
   @2    2020-08-01 0h 8m   [SE-1] 'test description'
   @1    2020-08-01 0h 5m   [SE-2] 'work on something else'
:: Total time: 0h 13m
:: Confirm upload [y/N]
```

It will print the worklogs it will upload, ask for confirmation.
On confirmation, it will upload the worklogs and mark them as logged locally in timewarrior.
See source code for the tags used with timewarrior.

## Dev

This is a standard cargo project, where you can use all the standard `cargo run|build|test|...` commands.
The api is also documented; generate the html docs with `cargo doc`.

Future work may include making the timewarrior tags, etc. that it uses configurable,
and supporting other time tracking tools (toggl, etc.).
The code is also structured in a way that makes it possible to use the timewarrior parsing tools and jira/tempo client in other projects, by importing this crate as a library.
For now, it works for me, so I probably won't do much more work on it apart from fixing bugs or keeping it up to date with my workflow.
If you're interested in using this tool/library and have requests, feel free to open an issue and/or a pull request! :)

## License

Copyright © 2020 Samuel Walladge

Dual licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
