# lava_torrent

[![crates.io](https://img.shields.io/crates/v/lava_torrent.svg)](https://crates.io/crates/lava_torrent)
[![Build Status](https://github.com/ttlajus/lava_torrent/actions/workflows/rust.yml/badge.svg)](https://github.com/ttlajus/lava_torrent/actions/workflows/rust.yml)
[![codecov](https://codecov.io/gh/ttlajus/lava_torrent/branch/master/graph/badge.svg)](https://codecov.io/gh/ttlajus/lava_torrent)

`lava_torrent` is a library for parsing/encoding/creating bencode and *.torrent* files.

## *Quick Start*
Read a torrent ([v1]) and print it and its info hash.

```rust
use lava_torrent::torrent::v1::Torrent;

let torrent = Torrent::read_from_file("sample.torrent").unwrap();
println!("{}", torrent);
println!("Info hash: {}", torrent.info_hash());
```

Create a torrent ([v1]) from files in a directory and save the *.torrent* file.

```rust
use lava_torrent::torrent::v1::TorrentBuilder;

let torrent = TorrentBuilder::new("dir/", 1048576).build().unwrap();
torrent.write_into_file("sample.torrent").unwrap();
```

Bencode (de)serialization.

```rust
use lava_torrent::bencode::BencodeElem;

let bytes = "d4:spam4:eggse".as_bytes();
let dict = BencodeElem::Dictionary([("spam".to_owned(), "eggs".into())].into());

assert_eq!(BencodeElem::from_bytes(bytes).unwrap()[0], dict);
assert_eq!(dict.encode(), bytes);

assert!(dict.write_into_file("/tmp/foo").is_ok());
assert_eq!(BencodeElem::from_file("/tmp/foo").unwrap()[0], dict);
```

## *More Info*
Please check the [documentation].

[v1]: http://bittorrent.org/beps/bep_0003.html
[documentation]: https://docs.rs/lava_torrent/

License: MIT OR Apache-2.0
