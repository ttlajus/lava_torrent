# lava_torrent

[![crates.io](https://img.shields.io/crates/v/lava_torrent.svg)](https://crates.io/crates/lava_torrent)
[![Build Status](https://travis-ci.org/ttlajus/lava_torrent.svg?branch=master)](https://travis-ci.org/ttlajus/lava_torrent)
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

let torrent = TorrentBuilder::new("announce".to_string(), "dir/", 1048576).build().unwrap();
torrent.write_into_file("sample.torrent").unwrap();
```

## *More Info*
Please check the [documentation].

[v1]: http://bittorrent.org/beps/bep_0003.html
[documentation]: https://docs.rs/lava_torrent/

License: MIT OR Apache-2.0
