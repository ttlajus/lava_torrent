use super::*;
use bencode::BencodeElem;
use std::io::{BufWriter, Write};

impl File {
    pub(crate) fn into_bencode_elem(self) -> BencodeElem {
        let mut result: HashMap<String, BencodeElem> = HashMap::new();

        result.insert("length".to_owned(), BencodeElem::Integer(self.length));
        result.insert(
            "path".to_owned(),
            BencodeElem::List(
                self.path
                    .iter()
                    .map(|component| BencodeElem::String(component.to_string_lossy().into_owned()))
                    .collect(),
            ),
        );

        if let Some(extra_fields) = self.extra_fields {
            result.extend(extra_fields);
        }

        BencodeElem::Dictionary(result)
    }
}

impl Torrent {
    /// Encode `self` as bencode and write the result to `dst`.
    pub fn write_into<W>(self, dst: &mut W) -> Result<()>
    where
        W: Write,
    {
        let mut result: HashMap<String, BencodeElem> = HashMap::new();
        let mut info: HashMap<String, BencodeElem> = HashMap::new();

        if let Some(announce) = self.announce {
            result.insert("announce".to_owned(), BencodeElem::String(announce));
        }

        if let Some(list) = self.announce_list {
            result.insert(
                "announce-list".to_owned(),
                BencodeElem::List(
                    list.into_iter()
                        .map(|tier| {
                            BencodeElem::List(
                                tier.into_iter()
                                    .map(BencodeElem::String) // url -> string
                                    .collect::<Vec<BencodeElem>>(),
                            )
                        })
                        .collect::<Vec<BencodeElem>>(),
                ),
            );
        }

        if let Some(files) = self.files {
            info.insert(
                "files".to_owned(),
                BencodeElem::List(
                    files
                        .into_iter()
                        .map(|file| file.into_bencode_elem())
                        .collect(),
                ),
            );
        } else {
            info.insert("length".to_owned(), BencodeElem::Integer(self.length));
        }

        info.insert("name".to_owned(), BencodeElem::String(self.name));
        info.insert(
            "piece length".to_owned(),
            BencodeElem::Integer(self.piece_length),
        );
        info.insert(
            "pieces".to_owned(),
            BencodeElem::Bytes(self.pieces.into_iter().flatten().collect()),
        );

        if let Some(extra_info_fields) = self.extra_info_fields {
            info.extend(extra_info_fields);
        }

        result.insert("info".to_owned(), BencodeElem::Dictionary(info));

        if let Some(extra_fields) = self.extra_fields {
            result.extend(extra_fields);
        }

        BencodeElem::Dictionary(result).write_into(dst)
    }

    /// Encode `self` as bencode and write the result to `path`.
    ///
    /// `path` must be the path to a file.
    ///
    /// "This function will create a file if it does
    /// not exist, and will truncate it if it does."
    ///
    /// Note: it is the client's responsibility to ensure
    /// that all directories in `path` actually exist (e.g.
    /// by calling [`create_dir_all`](https://doc.rust-lang.org/std/fs/fn.create_dir_all.html)).
    pub fn write_into_file<P>(self, path: P) -> Result<()>
    where
        P: AsRef<Path>,
    {
        let file = ::std::fs::File::create(&path)?;
        self.write_into(&mut BufWriter::new(&file))?;
        file.sync_all()?;
        Ok(())
    }

    /// Encode `self` as bencode and return the result in a `Vec`.
    pub fn encode(self) -> Result<Vec<u8>> {
        let mut result = Vec::new();
        self.write_into(&mut result)?;
        Ok(result)
    }
}

#[cfg(test)]
mod file_write_tests {
    use super::*;
    use std::iter::FromIterator;

    #[test]
    fn into_bencode_elem_ok() {
        let file = File {
            length: 42,
            path: PathBuf::from("dir1/dir2/file"),
            extra_fields: None,
        };

        assert_eq!(
            file.into_bencode_elem(),
            bencode_elem!({ ("length", 42), ("path", ["dir1", "dir2", "file"]) }),
        )
    }

    #[test]
    fn into_bencode_elem_with_extra_fields() {
        let file = File {
            length: 42,
            path: PathBuf::from("dir1/dir2/file"),
            extra_fields: Some(HashMap::from_iter(
                vec![("comment".to_owned(), bencode_elem!("no comment"))].into_iter(),
            )),
        };

        assert_eq!(
            file.into_bencode_elem(),
            bencode_elem!({
                ("length", 42),
                ("path", ["dir1", "dir2", "file"]),
                ("comment", "no comment"),
            })
        )
    }
}

#[cfg(test)]
mod torrent_write_tests {
    // @note: `write_into_file()` is not tested as it is
    // best left to integration tests (in `tests/`).
    use super::*;
    use std::iter::FromIterator;

    #[test]
    fn write_ok() {
        let torrent = Torrent {
            announce: Some("url".to_owned()),
            announce_list: None,
            length: 4,
            files: None,
            name: "sample".to_owned(),
            piece_length: 2,
            pieces: vec![vec![1, 2], vec![3, 4]],
            extra_fields: None,
            extra_info_fields: None,
        };
        let mut result = Vec::new();

        torrent.write_into(&mut result).unwrap();
        assert_eq!(
            result,
            bencode_elem!({
                ("announce", "url"),
                ("info", {
                    ("length", 4),
                    ("name", "sample"),
                    ("piece length", 2),
                    ("pieces", (1, 2, 3, 4)),
                })
            })
            .encode()
        );
    }

    #[test]
    fn write_with_announce_list() {
        let torrent = Torrent {
            announce: Some("url".to_owned()),
            announce_list: Some(vec![
                vec!["url1".to_owned(), "url2".to_owned()],
                vec!["url3".to_owned(), "url4".to_owned()],
            ]),
            length: 4,
            files: None,
            name: "sample".to_owned(),
            piece_length: 2,
            pieces: vec![vec![1, 2], vec![3, 4]],
            extra_fields: None,
            extra_info_fields: None,
        };
        let mut result = Vec::new();

        torrent.write_into(&mut result).unwrap();
        assert_eq!(
            result,
            bencode_elem!({
                ("announce", "url"),
                ("announce-list", [["url1", "url2"], ["url3", "url4"]]),
                ("info", {
                    ("length", 4),
                    ("name", "sample"),
                    ("piece length", 2),
                    ("pieces", (1, 2, 3, 4)),
                })
            })
            .encode()
        );
    }

    #[test]
    fn write_with_extra_fields() {
        let torrent = Torrent {
            announce: Some("url".to_owned()),
            announce_list: None,
            length: 4,
            files: None,
            name: "sample".to_owned(),
            piece_length: 2,
            pieces: vec![vec![1, 2], vec![3, 4]],
            extra_fields: Some(HashMap::from_iter(
                vec![
                    ("comment2".to_owned(), bencode_elem!("no comment")),
                    ("comment1".to_owned(), bencode_elem!("no comment")),
                ]
                .into_iter(),
            )),
            extra_info_fields: None,
        };
        let mut result = Vec::new();

        torrent.write_into(&mut result).unwrap();
        assert_eq!(
            result,
            bencode_elem!({
                ("announce", "url"),
                ("comment1", "no comment"),
                ("comment2", "no comment"),
                ("info", {
                    ("length", 4),
                    ("name", "sample"),
                    ("piece length", 2),
                    ("pieces", (1, 2, 3, 4)),
                })
            })
            .encode()
        );
    }

    #[test]
    fn write_with_extra_info_fields() {
        let torrent = Torrent {
            announce: Some("url".to_owned()),
            announce_list: None,
            length: 4,
            files: None,
            name: "sample".to_owned(),
            piece_length: 2,
            pieces: vec![vec![1, 2], vec![3, 4]],
            extra_fields: None,
            extra_info_fields: Some(HashMap::from_iter(
                vec![
                    ("comment2".to_owned(), bencode_elem!("no comment")),
                    ("comment1".to_owned(), bencode_elem!("no comment")),
                ]
                .into_iter(),
            )),
        };
        let mut result = Vec::new();

        torrent.write_into(&mut result).unwrap();
        assert_eq!(
            result,
            bencode_elem!({
                ("announce", "url"),
                ("info", {
                    ("comment1", "no comment"),
                    ("comment2", "no comment"),
                    ("length", 4),
                    ("name", "sample"),
                    ("piece length", 2),
                    ("pieces", (1, 2, 3, 4)),
                })
            })
            .encode()
        );
    }

    #[test]
    fn write_with_multiple_files() {
        let torrent = Torrent {
            announce: Some("url".to_owned()),
            announce_list: None,
            length: 4,
            files: Some(vec![
                File {
                    length: 2,
                    path: PathBuf::from("dir1/dir2/file1"),
                    extra_fields: None,
                },
                File {
                    length: 2,
                    path: PathBuf::from("dir1/dir2/file2"),
                    extra_fields: None,
                },
            ]),
            name: "sample".to_owned(),
            piece_length: 2,
            pieces: vec![vec![1, 2], vec![3, 4]],
            extra_fields: None,
            extra_info_fields: None,
        };
        let mut result = Vec::new();

        torrent.write_into(&mut result).unwrap();
        assert_eq!(
            result,
            bencode_elem!({
                ("announce", "url"),
                ("info", {
                    ("files", [
                        { ("length", 2), ("path", ["dir1", "dir2", "file1"]) },
                        { ("length", 2), ("path", ["dir1", "dir2", "file2"]) },
                    ]),
                    ("name", "sample"),
                    ("piece length", 2),
                    ("pieces", (1, 2, 3, 4)),
                })
            })
            .encode()
        );
    }

    #[test]
    fn encode_ok() {
        let torrent = Torrent {
            announce: Some("url".to_owned()),
            announce_list: None,
            length: 4,
            files: None,
            name: "sample".to_owned(),
            piece_length: 2,
            pieces: vec![vec![1, 2], vec![3, 4]],
            extra_fields: None,
            extra_info_fields: None,
        };

        assert_eq!(
            torrent.encode().unwrap(),
            bencode_elem!({
                ("announce", "url"),
                ("info", {
                    ("length", 4),
                    ("name", "sample"),
                    ("piece length", 2),
                    ("pieces", (1, 2, 3, 4)),
                })
            })
            .encode()
        );
    }

    #[test]
    fn encode_with_announce_list() {
        let torrent = Torrent {
            announce: Some("url".to_owned()),
            announce_list: Some(vec![
                vec!["url1".to_owned(), "url2".to_owned()],
                vec!["url3".to_owned(), "url4".to_owned()],
            ]),
            length: 4,
            files: None,
            name: "sample".to_owned(),
            piece_length: 2,
            pieces: vec![vec![1, 2], vec![3, 4]],
            extra_fields: None,
            extra_info_fields: None,
        };

        assert_eq!(
            torrent.encode().unwrap(),
            bencode_elem!({
                ("announce", "url"),
                ("announce-list", [["url1", "url2"], ["url3", "url4"]]),
                ("info", {
                    ("length", 4),
                    ("name", "sample"),
                    ("piece length", 2),
                    ("pieces", (1, 2, 3, 4)),
                })
            })
            .encode()
        );
    }

    #[test]
    fn encode_with_extra_fields() {
        let torrent = Torrent {
            announce: Some("url".to_owned()),
            announce_list: None,
            length: 4,
            files: None,
            name: "sample".to_owned(),
            piece_length: 2,
            pieces: vec![vec![1, 2], vec![3, 4]],
            extra_fields: Some(HashMap::from_iter(
                vec![
                    ("comment2".to_owned(), bencode_elem!("no comment")),
                    ("comment1".to_owned(), bencode_elem!("no comment")),
                ]
                .into_iter(),
            )),
            extra_info_fields: None,
        };

        assert_eq!(
            torrent.encode().unwrap(),
            bencode_elem!({
                ("announce", "url"),
                ("comment1", "no comment"),
                ("comment2", "no comment"),
                ("info", {
                    ("length", 4),
                    ("name", "sample"),
                    ("piece length", 2),
                    ("pieces", (1, 2, 3, 4)),
                })
            })
            .encode()
        );
    }

    #[test]
    fn encode_with_extra_info_fields() {
        let torrent = Torrent {
            announce: Some("url".to_owned()),
            announce_list: None,
            length: 4,
            files: None,
            name: "sample".to_owned(),
            piece_length: 2,
            pieces: vec![vec![1, 2], vec![3, 4]],
            extra_fields: None,
            extra_info_fields: Some(HashMap::from_iter(
                vec![
                    ("comment2".to_owned(), bencode_elem!("no comment")),
                    ("comment1".to_owned(), bencode_elem!("no comment")),
                ]
                .into_iter(),
            )),
        };

        assert_eq!(
            torrent.encode().unwrap(),
            bencode_elem!({
                ("announce", "url"),
                ("info", {
                    ("comment1", "no comment"),
                    ("comment2", "no comment"),
                    ("length", 4),
                    ("name", "sample"),
                    ("piece length", 2),
                    ("pieces", (1, 2, 3, 4)),
                })
            })
            .encode()
        );
    }

    #[test]
    fn encode_with_multiple_files() {
        let torrent = Torrent {
            announce: Some("url".to_owned()),
            announce_list: None,
            length: 4,
            files: Some(vec![
                File {
                    length: 2,
                    path: PathBuf::from("dir1/dir2/file1"),
                    extra_fields: None,
                },
                File {
                    length: 2,
                    path: PathBuf::from("dir1/dir2/file2"),
                    extra_fields: None,
                },
            ]),
            name: "sample".to_owned(),
            piece_length: 2,
            pieces: vec![vec![1, 2], vec![3, 4]],
            extra_fields: None,
            extra_info_fields: None,
        };

        assert_eq!(
            torrent.encode().unwrap(),
            bencode_elem!({
                ("announce", "url"),
                ("info", {
                    ("files", [
                        { ("length", 2), ("path", ["dir1", "dir2", "file1"]) },
                        { ("length", 2), ("path", ["dir1", "dir2", "file2"]) },
                    ]),
                    ("name", "sample"),
                    ("piece length", 2),
                    ("pieces", (1, 2, 3, 4)),
                })
            })
            .encode()
        );
    }
}
