# cpplumber [![License](https://img.shields.io/badge/license-GPL--3.0-blue.svg)](https://img.shields.io/badge/license-GPL--3.0-blue.svg) [![rustc 1.63.0](https://img.shields.io/badge/rust-1.63.0%2B-orange.svg)](https://img.shields.io/badge/rust-1.63.0%2B-orange.svg)

Cpplumber is a static analysis tool that helps detecting and keeping track of C
and C++ source code information that leaks into compiled executable files.

## Documentation

The documentation is available [here](https://github.com/ergrelet/cpplumber/blob/gh-pages/index.md).

## How to Build

Rust version 1.63.0 or greater is needed to build the project.

```
git clone https://github.com/ergrelet/cpplumber.git
cd cpplumber
cargo build --release
```
