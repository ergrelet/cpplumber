# cpplumber [![License](https://img.shields.io/badge/license-GPL--3.0-blue.svg)](https://img.shields.io/badge/license-GPL--3.0-blue.svg) [![rustc 1.63.0](https://img.shields.io/badge/rust-1.63.0%2B-orange.svg)](https://img.shields.io/badge/rust-1.63.0%2B-orange.svg) [![Tests Status](https://github.com/ergrelet/cpplumber/workflows/Tests/badge.svg?branch=main)](https://github.com/ergrelet/cpplumber/actions?query=workflow%3ATests)

Cpplumber is a static analysis tool that helps detecting and keeping track of C
and C++ source code information that leaks into compiled executable files.

The project is written in Rust and depends on libclang, so it's cross-platform and
can be used on projects that use the latest C and C++ standards.

## Key Features

* Supports JSON compilation databases
* Tracks leaks of string literals, struct names and class names
* Allows filtering reported leaks through a YAML configuration file
* Generates raw text and JSON reports

## Quick Example

Imagine you have a source file `file1.c` that you compiled into `a.out` and
you want to know if some string literal ended up in `a.out`, you can simply do:
```
$ cpplumber --bin a.out file1.c
[2022-09-24T19:57:14Z INFO  cpplumber] Gathering source files...
[2022-09-24T19:57:14Z INFO  cpplumber] Filtering suppressed files...
[2022-09-24T19:57:14Z INFO  cpplumber] Extracting artifacts from source files...
[2022-09-24T19:57:15Z INFO  cpplumber] Filtering suppressed artifacts...
[2022-09-24T19:57:15Z INFO  cpplumber] Looking for leaks in 'a.out'...
"My_Super_Secret_API_Key" (string literal) leaked at offset 0x14f20 in "/full/path/to/a.out" [declared at /full/path/to/file1.c:5]
Error: Leaks detected!
```

## Documentation

The full user documentation is available [here](https://github.com/ergrelet/cpplumber/blob/gh-pages/index.md).

## How to Build

Rust version 1.63.0 or greater is needed to build the project.

```
git clone https://github.com/ergrelet/cpplumber.git
cd cpplumber
cargo build --release
```
