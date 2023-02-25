# moove - 🚚 Manipulate file names and locations

[![Version][image-version]][url-version]
[![Downloads from crates.io][image-downloads-crates]][url-crates.io]
[![Downloads from GitHub][image-downloads-github]][url-latest-release]
[![License][image-license]](#license-)

*moove* is a command line tool for renaming and moving files and directories using a text editor.

[🎨 Features](#features-) \|
[🚀 Usage](#usage-) \|
[📥 Getting started](#getting-started-) \|
[💙 Contributing](#contributing-) \|
[🌏 License](#license-)

![Demo](https://raw.githubusercontent.com/urin/moove/main/README-demo.webp)

## Features 🎨

- Displays file and directory names like [`ls`](https://man7.org/linux/man-pages/man1/ls.1.html) in a text editor,
  and renames or moves them exactly as you edit them.
- A pre-compiled single executable without any dependencies.
  Thanks 💖 to [Zig](https://ziglang.org/) and [musl libc](https://musl.libc.org/).
- Supports Linux, Mac, and Windows.
- Supports wildcard patterns, including Windows.

### Caveats ⚠

- Given paths have to be convertible to UTF-8.
- Collisions are detected *as much as possible*, but *not perfectly*.
  Does not verify all paths such as hard links and symbolic links.

## Usage 🚀

```txt
Usage: moove [OPTIONS] [PATHS]...

Arguments:
  [PATHS]...  Paths or wildcard patterns to move

Options:
  -v, --verbose                    Verbose output
  -s, --sort                       Sort in the natural order
  -a, --absolute                   Treat as absolute paths
  -d, --directory                  Directories themselves, not their contents
  -w, --with-hidden                Include hidden files
  -e, --exclude-pattern <PATTERN>  Exclude regular expression pattern
  -c, --copy                       Copy without moving
  -u, --dry-run                    Dry-run
  -o, --oops                       Abort in case of collision (prompt as default)
  -q, --quiet                      No output to stdout/strerr even if error
  -h, --help                       Print help
  -V, --version                    Print version
```

- Displays file and directory names like [`ls`](https://man7.org/linux/man-pages/man1/ls.1.html) in a text editor.
- You can edit the list as you want to operate. The order of lines after editing corresponds to the original one. Empty lines will be ignored.
- Operations are canceled if you close the editor without saving.
- If a line starts with `//`, the file and directory (and its contents) will be removed regardless of modification of the remaining part of the line.
- Destination directories will be created automatically.
- In case of line number change or collision, asks whether to re-edit or abort. Aborts without asking if `--oops` is specified.

### Configuration 🎚

- Default command line options can be specified by the environment variable `MOOVE_OPTIONS`.
- The default editor is searched in the following order.
  - environment variable `VISUAL`
  - environment variable `EDITOR`
  - hardcoded lists
  - platform-specific generic file openers

## Getting Started 📥

### Pre-compiled binaries

- [moove-apple-aarch64.tar.gz](https://github.com/urin/moove/releases/latest/download/moove-apple-aarch64.tar.gz)
- [moove-apple-x86_64.tar.gz](https://github.com/urin/moove/releases/latest/download/moove-apple-x86_64.tar.gz)
- [moove-linux-aarch64.tar.gz](https://github.com/urin/moove/releases/latest/download/moove-linux-aarch64.tar.gz)
- [moove-linux-x86_64.tar.gz](https://github.com/urin/moove/releases/latest/download/moove-linux-x86_64.tar.gz)
- [moove-windows-x86_64.tar.gz](https://github.com/urin/moove/releases/latest/download/moove-windows-x86_64.tar.gz)

### Install by cargo

```sh
cargo install moove
```

## Alternatives

- [laurent22/massren](https://github.com/laurent22/massren)
- [itchyny/mmv](https://github.com/itchyny/mmv)

## Contributing 💙

Followings are used to build.

- [cargo-make](https://crates.io/crates/cargo-make/) as the task runner
- [cargo-zigbuild](https://crates.io/crates/cargo-zigbuild) to build for multiple platforms

### Setup building environment 🪜

1. Install [Zig](https://ziglang.org/) according to [the Zig document](https://ziglang.org/learn/getting-started/#installing-zig).
2. Run following commands.
```sh
cargo install cargo-make
cargo make setup
```

### Testing and Building 🔨

- To test,
  ```txt
  cargo make test
  ```

- To build binaries for release,
  ```txt
  cargo make
  ```
  Pre-compiled binaries will be in the directory `dist`.

  ⚠  Binaries do not have execute permission in case of building on windows.

## TODOs ✅

- Package for various platforms
- Overwrite option
- Exclude .gitignore option
- Move to trash instead of removing as the default
- Logging
- Recursive option
- Maximum depth option
- Depth option

## License 🌏

Licensed under either of

- [Apache License, Version 2.0][url-license-apache] or
  [https://www.apache.org/licenses/LICENSE-2.0](https://www.apache.org/licenses/LICENSE-2.0)
- [MIT license][url-license-mit] or
  [https://opensource.org/licenses/MIT](https://opensource.org/licenses/MIT)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

© 2023 [Urin](https://github.com/urin)

<!-- Reference -->

[image-license]: https://img.shields.io/badge/license-MIT%2FApache--2.0-lightgrey?style=flat
[image-downloads-crates]: https://img.shields.io/crates/d/moove?label=downloads&style=flat
[image-downloads-github]: https://img.shields.io/github/downloads/urin/moove/total?label=from%20GitHub&style=flat
[image-version]: https://img.shields.io/crates/v/moove.svg?style=flat

[url-license-mit]: https://github.com/urin/moove/blob/main/LICENSE-MIT
[url-license-apache]: https://github.com/urin/moove/blob/main/LICENSE-APACHE
[url-latest-release]: https://github.com/urin/moove/releases/latest
[url-releases]: https://github.com/urin/moove/releases
[url-version]: https://crates.io/crates/moove/versions
[url-crates.io]: https://crates.io/crates/moove
