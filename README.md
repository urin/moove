# moove - 🪄 Manipulate file names and locations

[![License][image-license]](#license-)

*moove* is a command line tool for renaming and moving files and directories using a text editor.


[🎨 Features](#features-) \|
[🚀 Usage](#usage-) \|
[📥 Getting started](#getting-started-) \|
[💙 Contributing](#contributing-) \|
[🌏 License](#license-)


## Features 🎨

- Displays file and directory names like [`ls`](https://man7.org/linux/man-pages/man1/ls.1.html) in a text editor,
  and renames or moves them exactly as you edit them.
- Aborts entire operation in case of collisions.
- Supports Linux, Mac, and Windows.

### Caveats ⚠

- Given paths have to be convertible to UTF-8.
- Collisions are detected *as much as possible*, but *not perfectly*.
  Does not verify all paths such as hard links and symbolic links.

## Usage 🚀

```txt
Usage: moove [OPTIONS] [PATHS]...

Arguments:
  [PATHS]...  Paths to move [default: .]

Options:
  -u, --dry-run    Dry-run option
  -v, --verbose    Verbose output
  -q, --quiet      No output to stdout/strerr even if error
  -a, --absolute   Handle as absolute paths
  -d, --directory  Directories themselves, not their contents
  -h, --help       Print help
  -V, --version    Print version
```

## Getting Started 📥

- Download pre-compiled binaries.
  - [moove-apple-aarch64.tar.gz](https://github.com/urin/moove/raw/main/dist/moove-apple-aarch64.tar.gz)
  - [moove-linux-aarch64.tar.gz](https://github.com/urin/moove/raw/main/dist/moove-linux-aarch64.tar.gz)
  - [moove-apple-x86_64.tar.gz](https://github.com/urin/moove/raw/main/dist/moove-apple-x86_64.tar.gz)
  - [moove-linux-x86_64.tar.gz](https://github.com/urin/moove/raw/main/dist/moove-linux-x86_64.tar.gz)
  - [moove-windows-x86_64.tar.gz](https://github.com/urin/moove/raw/main/dist/moove-windows-x86_64.tar.gz)

- Extract a single binary file from the archive file.
  For example,
  ```sh
  tar xaf moove-linux-x86_64.tar.gz
  ```

- Specify text editor configuring environment variable `VISUAL` or `EDITOR`.
  For example,
  ```sh
  export EDITOR=code
  ```

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

### Building 🔨

To build binaries for supported platforms,

```sh
cargo make
```

## TODOs ✅

- Overwrite option
- Order option
- Package for various platforms
- Recursive option
- Maximum depth option
- Depth option
- Enable applying pipe
- Remove operation
- Create option
- Exclude pattern option
- Exclude .gitignore option
- Exclude hidden files as default
- Configuration management
- Rollback option
- Log and undo

## License 🌏

Licensed under either of

- [Apache License, Version 2.0](url-license-apache) or
  [https://www.apache.org/licenses/LICENSE-2.0](https://www.apache.org/licenses/LICENSE-2.0)
- [MIT license](url-license-mit) or
  [https://opensource.org/licenses/MIT](https://opensource.org/licenses/MIT)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

© 2023 [Urin](https://github.com/urin)

<!-- Reference -->

[image-license]: https://img.shields.io/github/license/urin/moove?style=flat
[url-license-mit]: https://github.com/urin/moove/blob/main/LICENSE-MIT
[url-license-apache]: https://github.com/urin/moove/blob/main/LICENSE-APACHE

