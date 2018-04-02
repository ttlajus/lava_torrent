use std::io::Write;
use bencode::BencodeElem;
use super::*;

impl File {
    fn into_bencode_elem(self) -> Result<BencodeElem> {
        let mut result: HashMap<String, BencodeElem> = HashMap::new();

        result.insert("length".to_string(), BencodeElem::Integer(self.length));
        result.insert(
            "path".to_string(),
            BencodeElem::List({
                let mut list = Vec::new();
                for component in &self.path {
                    match component.to_str() {
                        Some(string) => list.push(BencodeElem::String(string.to_string())),
                        None => {
                            return Err(Error::new(
                                ErrorKind::MalformedTorrent,
                                Cow::Owned(format!(
                                    "Path component [{:?}] is not valid UTF8.",
                                    component
                                )),
                            ));
                        }
                    }
                }
                list
            }),
        );

        if let Some(extra_fields) = self.extra_fields {
            result.extend(extra_fields);
        }

        Ok(BencodeElem::Dictionary(result))
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

        result.insert("announce".to_string(), BencodeElem::String(self.announce));

        if let Some(list) = self.announce_list {
            result.insert(
                "announce-list".to_string(),
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
                "files".to_string(),
                BencodeElem::List({
                    let mut list = Vec::new();
                    for file in files {
                        list.push(file.into_bencode_elem()?);
                    }
                    list
                }),
            );
        } else {
            info.insert("length".to_string(), BencodeElem::Integer(self.length));
        }

        info.insert("name".to_string(), BencodeElem::String(self.name));
        info.insert(
            "piece length".to_string(),
            BencodeElem::Integer(self.piece_length),
        );
        info.insert(
            "pieces".to_string(),
            BencodeElem::Bytes(self.pieces.into_iter().flat_map(|piece| piece).collect()),
        );

        if let Some(extra_info_fields) = self.extra_info_fields {
            info.extend(extra_info_fields);
        }

        result.insert("info".to_string(), BencodeElem::Dictionary(info));

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
    pub fn write_into_file<P>(self, path: P) -> Result<()>
    where
        P: AsRef<Path>,
    {
        match ::std::fs::File::create(&path) {
            Ok(mut file) => {
                self.write_into(&mut file)?;
                file.sync_all()?;
                Ok(())
            }
            Err(_) => Err(Error::new(
                ErrorKind::IOError,
                Cow::Owned(format!("Failed to create [{}].", path.as_ref().display())),
            )),
        }
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

        match file.into_bencode_elem() {
            Ok(encoded) => assert_eq!(
                encoded,
                bencode_elem!({ ("length", 42), ("path", ["dir1", "dir2", "file"]) })
            ),
            Err(_) => assert!(false),
        }
    }

    #[test]
    fn into_bencode_elem_with_extra_fields() {
        let file = File {
            length: 42,
            path: PathBuf::from("dir1/dir2/file"),
            extra_fields: Some(HashMap::from_iter(
                vec![("comment".to_string(), bencode_elem!("no comment"))].into_iter(),
            )),
        };

        match file.into_bencode_elem() {
            Ok(elem) => assert_eq!(
                elem,
                bencode_elem!({
                    ("length", 42),
                    ("path", ["dir1", "dir2", "file"]),
                    ("comment", "no comment"),
                })
            ),
            Err(_) => assert!(false),
        }
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
            announce: "url".to_string(),
            announce_list: None,
            length: 4,
            files: None,
            name: "sample".to_string(),
            piece_length: 2,
            pieces: vec![vec![1, 2], vec![3, 4]],
            extra_fields: None,
            extra_info_fields: None,
            encoded_info: Vec::new(),
        };
        let mut result = Vec::new();

        match torrent.write_into(&mut result) {
            Ok(_) => assert_eq!(
                result,
                bencode_elem!({
                    ("announce", "url"),
                    ("info", {
                        ("length", 4),
                        ("name", "sample"),
                        ("piece length", 2),
                        ("pieces", (1, 2, 3, 4)),
                    })
                }).encode()
            ),
            Err(_) => assert!(false),
        }
    }

    #[test]
    fn write_with_announce_list() {
        let torrent = Torrent {
            announce: "url".to_string(),
            announce_list: Some(vec![
                vec!["url1".to_string(), "url2".to_string()],
                vec!["url3".to_string(), "url4".to_string()],
            ]),
            length: 4,
            files: None,
            name: "sample".to_string(),
            piece_length: 2,
            pieces: vec![vec![1, 2], vec![3, 4]],
            extra_fields: None,
            extra_info_fields: None,
            encoded_info: Vec::new(),
        };
        let mut result = Vec::new();

        match torrent.write_into(&mut result) {
            Ok(_) => assert_eq!(
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
                }).encode()
            ),
            Err(_) => assert!(false),
        }
    }

    #[test]
    fn write_with_extra_fields() {
        let torrent = Torrent {
            announce: "url".to_string(),
            announce_list: None,
            length: 4,
            files: None,
            name: "sample".to_string(),
            piece_length: 2,
            pieces: vec![vec![1, 2], vec![3, 4]],
            extra_fields: Some(HashMap::from_iter(
                vec![
                    ("comment2".to_string(), bencode_elem!("no comment")),
                    ("comment1".to_string(), bencode_elem!("no comment")),
                ].into_iter(),
            )),
            extra_info_fields: None,
            encoded_info: Vec::new(),
        };
        let mut result = Vec::new();

        match torrent.write_into(&mut result) {
            Ok(_) => assert_eq!(
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
                }).encode()
            ),
            Err(_) => assert!(false),
        }
    }

    #[test]
    fn write_with_extra_info_fields() {
        let torrent = Torrent {
            announce: "url".to_string(),
            announce_list: None,
            length: 4,
            files: None,
            name: "sample".to_string(),
            piece_length: 2,
            pieces: vec![vec![1, 2], vec![3, 4]],
            extra_fields: None,
            extra_info_fields: Some(HashMap::from_iter(
                vec![
                    ("comment2".to_string(), bencode_elem!("no comment")),
                    ("comment1".to_string(), bencode_elem!("no comment")),
                ].into_iter(),
            )),
            encoded_info: Vec::new(),
        };
        let mut result = Vec::new();

        match torrent.write_into(&mut result) {
            Ok(_) => assert_eq!(
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
                }).encode()
            ),
            Err(_) => assert!(false),
        }
    }

    #[test]
    fn write_with_multiple_files() {
        let torrent = Torrent {
            announce: "url".to_string(),
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
            name: "sample".to_string(),
            piece_length: 2,
            pieces: vec![vec![1, 2], vec![3, 4]],
            extra_fields: None,
            extra_info_fields: None,
            encoded_info: Vec::new(),
        };
        let mut result = Vec::new();

        match torrent.write_into(&mut result) {
            Ok(_) => assert_eq!(
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
                }).encode()
            ),
            Err(_) => assert!(false),
        }
    }

    #[test]
    fn encode_ok() {
        let torrent = Torrent {
            announce: "url".to_string(),
            announce_list: None,
            length: 4,
            files: None,
            name: "sample".to_string(),
            piece_length: 2,
            pieces: vec![vec![1, 2], vec![3, 4]],
            extra_fields: None,
            extra_info_fields: None,
            encoded_info: Vec::new(),
        };

        match torrent.encode() {
            Ok(encoded) => assert_eq!(
                encoded,
                bencode_elem!({
                    ("announce", "url"),
                    ("info", {
                        ("length", 4),
                        ("name", "sample"),
                        ("piece length", 2),
                        ("pieces", (1, 2, 3, 4)),
                    })
                }).encode()
            ),
            Err(_) => assert!(false),
        }
    }

    #[test]
    fn encode_with_announce_list() {
        let torrent = Torrent {
            announce: "url".to_string(),
            announce_list: Some(vec![
                vec!["url1".to_string(), "url2".to_string()],
                vec!["url3".to_string(), "url4".to_string()],
            ]),
            length: 4,
            files: None,
            name: "sample".to_string(),
            piece_length: 2,
            pieces: vec![vec![1, 2], vec![3, 4]],
            extra_fields: None,
            extra_info_fields: None,
            encoded_info: Vec::new(),
        };

        match torrent.encode() {
            Ok(encoded) => assert_eq!(
                encoded,
                bencode_elem!({
                    ("announce", "url"),
                    ("announce-list", [["url1", "url2"], ["url3", "url4"]]),
                    ("info", {
                        ("length", 4),
                        ("name", "sample"),
                        ("piece length", 2),
                        ("pieces", (1, 2, 3, 4)),
                    })
                }).encode()
            ),
            Err(_) => assert!(false),
        }
    }

    #[test]
    fn encode_with_extra_fields() {
        let torrent = Torrent {
            announce: "url".to_string(),
            announce_list: None,
            length: 4,
            files: None,
            name: "sample".to_string(),
            piece_length: 2,
            pieces: vec![vec![1, 2], vec![3, 4]],
            extra_fields: Some(HashMap::from_iter(
                vec![
                    ("comment2".to_string(), bencode_elem!("no comment")),
                    ("comment1".to_string(), bencode_elem!("no comment")),
                ].into_iter(),
            )),
            extra_info_fields: None,
            encoded_info: Vec::new(),
        };

        match torrent.encode() {
            Ok(encoded) => assert_eq!(
                encoded,
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
                }).encode()
            ),
            Err(_) => assert!(false),
        }
    }

    #[test]
    fn encode_with_extra_info_fields() {
        let torrent = Torrent {
            announce: "url".to_string(),
            announce_list: None,
            length: 4,
            files: None,
            name: "sample".to_string(),
            piece_length: 2,
            pieces: vec![vec![1, 2], vec![3, 4]],
            extra_fields: None,
            extra_info_fields: Some(HashMap::from_iter(
                vec![
                    ("comment2".to_string(), bencode_elem!("no comment")),
                    ("comment1".to_string(), bencode_elem!("no comment")),
                ].into_iter(),
            )),
            encoded_info: Vec::new(),
        };

        match torrent.encode() {
            Ok(encoded) => assert_eq!(
                encoded,
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
                }).encode()
            ),
            Err(_) => assert!(false),
        }
    }

    #[test]
    fn encode_with_multiple_files() {
        let torrent = Torrent {
            announce: "url".to_string(),
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
            name: "sample".to_string(),
            piece_length: 2,
            pieces: vec![vec![1, 2], vec![3, 4]],
            extra_fields: None,
            extra_info_fields: None,
            encoded_info: Vec::new(),
        };

        match torrent.encode() {
            Ok(encoded) => assert_eq!(
                encoded,
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
                }).encode()
            ),
            Err(_) => assert!(false),
        }
    }
}
