use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::borrow::Cow;
use bencode::BencodeElem;
use util;
use super::*;

impl File {
    fn extract_file(elem: BencodeElem) -> Result<File> {
        match elem {
            BencodeElem::Dictionary(mut dict) => Ok(File {
                length: Self::extract_file_length(&mut dict)?,
                path: Self::extract_file_path(&mut dict)?,
                extra_fields: Self::extract_file_extra_fields(dict),
            }),
            _ => Err(Error::new(
                ErrorKind::MalformedTorrent,
                Cow::Borrowed("\"files\" contains a non-dictionary element."),
            )),
        }
    }

    fn extract_file_length(dict: &mut HashMap<String, BencodeElem>) -> Result<i64> {
        match dict.remove("length") {
            Some(BencodeElem::Integer(len)) => {
                if len > 0 {
                    Ok(len)
                } else {
                    Err(Error::new(
                        ErrorKind::MalformedTorrent,
                        Cow::Borrowed("\"length\" <= 0."),
                    ))
                }
            }
            Some(_) => Err(Error::new(
                ErrorKind::MalformedTorrent,
                Cow::Borrowed("\"length\" does not map to an integer."),
            )),
            None => Err(Error::new(
                ErrorKind::MalformedTorrent,
                Cow::Borrowed("\"length\" does not exist."),
            )),
        }
    }

    fn extract_file_path(dict: &mut HashMap<String, BencodeElem>) -> Result<PathBuf> {
        match dict.remove("path") {
            Some(BencodeElem::List(list)) => {
                if list.is_empty() {
                    Err(Error::new(
                        ErrorKind::MalformedTorrent,
                        Cow::Borrowed("\"path\" maps to a 0-length list."),
                    ))
                } else {
                    let mut path = PathBuf::new();
                    for component in list {
                        if let BencodeElem::String(component) = component {
                            // "Path components exactly matching '.' and '..'
                            // must be sanitized. This sanitizing step must
                            // happen after normalizing overlong UTF-8 encodings."
                            // Rust rejects overlong encodings, and NFC
                            // normalization is performed when parsing bencode.
                            if (component == ".") || (component == "..") {
                                return Err(Error::new(
                                    ErrorKind::MalformedTorrent,
                                    Cow::Borrowed("\"path\" contains \".\" or \"..\"."),
                                ));
                            } else {
                                path.push(component);
                            }
                        } else {
                            return Err(Error::new(
                                ErrorKind::MalformedTorrent,
                                Cow::Borrowed("\"path\" contains a non-string element."),
                            ));
                        }
                    }
                    Ok(path)
                }
            }
            Some(_) => Err(Error::new(
                ErrorKind::MalformedTorrent,
                Cow::Borrowed("\"path\" does not map to a list."),
            )),
            None => Err(Error::new(
                ErrorKind::MalformedTorrent,
                Cow::Borrowed("\"path\" does not exist."),
            )),
        }
    }

    fn extract_file_extra_fields(dict: HashMap<String, BencodeElem>) -> Option<Dictionary> {
        if dict.is_empty() {
            None
        } else {
            Some(dict)
        }
    }
}

impl Torrent {
    /// Parse `bytes` and return the extracted `Torrent`.
    ///
    /// If `bytes` is missing any required field (e.g. `info`), or if any other
    /// error is encountered (e.g. `IOError`), then `Err(error)` will be returned.
    pub fn read_from_bytes<B>(bytes: B) -> Result<Torrent>
    where
        B: AsRef<[u8]>,
    {
        Self::from_parsed(BencodeElem::from_bytes(bytes)?)?.validate()
    }

    /// Parse the content of the file at `path` and return the extracted `Torrent`.
    ///
    /// If the file at `path` is missing any required field (e.g. `info`), or if any other
    /// error is encountered (e.g. `IOError`), then `Err(error)` will be returned.
    pub fn read_from_file<P>(path: P) -> Result<Torrent>
    where
        P: AsRef<Path>,
    {
        Self::from_parsed(BencodeElem::from_file(path)?)?.validate()
    }

    // @note: Most of validation is done when bdecoding and parsing torrent,
    // so there's not much going on here. More validation could be
    // added in the future if necessary.
    fn validate(self) -> Result<Torrent> {
        if let Some(total_piece_length) =
            util::i64_to_usize(self.piece_length)?.checked_mul(self.pieces.len())
        {
            if total_piece_length < util::i64_to_usize(self.length)? {
                Err(Error::new(
                    ErrorKind::MalformedTorrent,
                    Cow::Owned(format!(
                        "Total piece length {} < torrent's length {}.",
                        total_piece_length, self.length,
                    )),
                ))
            } else if self.length <= 0 {
                Err(Error::new(
                    ErrorKind::MalformedTorrent,
                    Cow::Borrowed("\"length\" <= 0."),
                ))
            } else {
                Ok(self)
            }
        } else {
            Err(Error::new(
                ErrorKind::MalformedTorrent,
                Cow::Borrowed("Torrent's total piece length overflowed in usize."),
            ))
        }
    }

    fn from_parsed(mut parsed: Vec<BencodeElem>) -> Result<Torrent> {
        if parsed.len() != 1 {
            return Err(Error::new(
                ErrorKind::MalformedTorrent,
                Cow::Owned(format!(
                    "Torrent should contain 1 and only 1 top-level element, {} found.",
                    parsed.len()
                )),
            ));
        }

        if let BencodeElem::Dictionary(mut parsed) = parsed.remove(0) {
            // 2nd-level items
            let announce = Self::extract_announce(&mut parsed)?;
            let announce_list = Self::extract_announce_list(&mut parsed)?;
            let info = parsed.remove("info");
            let extra_fields = Self::extract_extra_fields(parsed);

            match info {
                Some(BencodeElem::Dictionary(mut info)) => {
                    // 3rd-level items
                    // handle `files` separately because `extract_length()` needs it
                    let files = Self::extract_files(&mut info)?;

                    Ok(Torrent {
                        announce,
                        announce_list,
                        length: Self::extract_length(&mut info, &files)?,
                        files,
                        name: Self::extract_name(&mut info)?,
                        piece_length: Self::extract_piece_length(&mut info)?,
                        pieces: Self::extract_pieces(&mut info)?,
                        extra_fields,
                        extra_info_fields: Self::extract_extra_fields(info),
                    })
                }
                Some(_) => Err(Error::new(
                    ErrorKind::MalformedTorrent,
                    Cow::Borrowed("\"info\" is not a dictionary."),
                )),
                None => Err(Error::new(
                    ErrorKind::MalformedTorrent,
                    Cow::Borrowed("\"info\" does not exist."),
                )),
            }
        } else {
            Err(Error::new(
                ErrorKind::MalformedTorrent,
                Cow::Borrowed("Torrent's top-level element is not a dictionary."),
            ))
        }
    }

    fn extract_announce(dict: &mut HashMap<String, BencodeElem>) -> Result<String> {
        match dict.remove("announce") {
            Some(BencodeElem::String(url)) => Ok(url),
            Some(_) => Err(Error::new(
                ErrorKind::MalformedTorrent,
                Cow::Borrowed("\"announce\" does not map to a string (or maps to invalid UTF8)."),
            )),
            None => Err(Error::new(
                ErrorKind::MalformedTorrent,
                Cow::Borrowed("\"announce\" does not exist."),
            )),
        }
    }

    fn extract_announce_list(
        dict: &mut HashMap<String, BencodeElem>,
    ) -> Result<Option<AnnounceList>> {
        let mut announce_list = Vec::new();

        match dict.remove("announce-list") {
            Some(BencodeElem::List(tiers)) => {
                for tier in tiers {
                    announce_list.push(Self::extract_announce_list_tier(tier)?);
                }
                Ok(Some(announce_list))
            }
            Some(_) => Err(Error::new(
                ErrorKind::MalformedTorrent,
                Cow::Borrowed("\"announce-list\" does not map to a list."),
            )),
            // Since BEP 12 is an extension,
            // the existence of `announce-list` is not guaranteed.
            None => Ok(None),
        }
    }

    fn extract_announce_list_tier(elem: BencodeElem) -> Result<Vec<String>> {
        match elem {
            BencodeElem::List(urls) => {
                let mut tier = Vec::new();
                for url in urls {
                    match url {
                        BencodeElem::String(url) => tier.push(url),
                        _ => {
                            return Err(Error::new(
                                ErrorKind::MalformedTorrent,
                                Cow::Borrowed(
                                    "A tier within \"announce-list\" \
                                     contains a non-string element.",
                                ),
                            ));
                        }
                    }
                }
                Ok(tier)
            }
            _ => Err(Error::new(
                ErrorKind::MalformedTorrent,
                Cow::Borrowed("\"announce-list\" contains a non-list element."),
            )),
        }
    }

    fn extract_files(dict: &mut HashMap<String, BencodeElem>) -> Result<Option<Vec<File>>> {
        match dict.remove("files") {
            Some(BencodeElem::List(list)) => {
                if list.is_empty() {
                    Err(Error::new(
                        ErrorKind::MalformedTorrent,
                        Cow::Borrowed("\"files\" maps to an empty list."),
                    ))
                } else {
                    let mut files = Vec::new();
                    for file in list {
                        files.push(File::extract_file(file)?);
                    }
                    Ok(Some(files))
                }
            }
            Some(_) => Err(Error::new(
                ErrorKind::MalformedTorrent,
                Cow::Borrowed("\"files\" does not map to a list."),
            )),
            None => Ok(None),
        }
    }

    fn extract_length(
        dict: &mut HashMap<String, BencodeElem>,
        files: &Option<Vec<File>>,
    ) -> Result<i64> {
        match dict.remove("length") {
            Some(BencodeElem::Integer(len)) => {
                if files.is_some() {
                    Err(Error::new(
                        ErrorKind::MalformedTorrent,
                        Cow::Borrowed("Both \"length\" and \"files\" exist."),
                    ))
                } else {
                    Ok(len)
                }
            }
            Some(_) => Err(Error::new(
                ErrorKind::MalformedTorrent,
                Cow::Borrowed("\"length\" does not map to an integer."),
            )),
            None => {
                if let Some(ref files) = *files {
                    let mut length: i64 = 0;
                    for file in files {
                        match length.checked_add(file.length) {
                            Some(sum) => {
                                length = sum;
                            }
                            None => {
                                return Err(Error::new(
                                    ErrorKind::MalformedTorrent,
                                    Cow::Borrowed("Torrent's length overflowed in i64."),
                                ));
                            }
                        }
                    }
                    Ok(length)
                } else {
                    Err(Error::new(
                        ErrorKind::MalformedTorrent,
                        Cow::Borrowed("Neither \"length\" nor \"files\" exists."),
                    ))
                }
            }
        }
    }

    fn extract_name(dict: &mut HashMap<String, BencodeElem>) -> Result<String> {
        match dict.remove("name") {
            Some(BencodeElem::String(name)) => Ok(name),
            Some(_) => Err(Error::new(
                ErrorKind::MalformedTorrent,
                Cow::Borrowed("\"name\" does not map to a string (or maps to invalid UTF8)."),
            )),
            None => Err(Error::new(
                ErrorKind::MalformedTorrent,
                Cow::Borrowed("\"name\" does not exist."),
            )),
        }
    }

    fn extract_piece_length(dict: &mut HashMap<String, BencodeElem>) -> Result<i64> {
        match dict.remove("piece length") {
            Some(BencodeElem::Integer(len)) => {
                if len > 0 {
                    Ok(len)
                } else {
                    Err(Error::new(
                        ErrorKind::MalformedTorrent,
                        Cow::Borrowed("\"piece length\" <= 0."),
                    ))
                }
            }
            Some(_) => Err(Error::new(
                ErrorKind::MalformedTorrent,
                Cow::Borrowed("\"piece length\" does not map to an integer."),
            )),
            None => Err(Error::new(
                ErrorKind::MalformedTorrent,
                Cow::Borrowed("\"piece length\" does not exist."),
            )),
        }
    }

    fn extract_pieces(dict: &mut HashMap<String, BencodeElem>) -> Result<Vec<Piece>> {
        match dict.remove("pieces") {
            Some(BencodeElem::Bytes(bytes)) => {
                if bytes.is_empty() {
                    Err(Error::new(
                        ErrorKind::MalformedTorrent,
                        Cow::Borrowed("\"pieces\" maps to an empty sequence."),
                    ))
                } else if (bytes.len() % PIECE_STRING_LENGTH) != 0 {
                    Err(Error::new(
                        ErrorKind::MalformedTorrent,
                        Cow::Owned(format!(
                            "\"pieces\"' length is not a multiple of {}.",
                            PIECE_STRING_LENGTH,
                        )),
                    ))
                } else {
                    Ok(bytes
                        .chunks(PIECE_STRING_LENGTH)
                        .map(|chunk| chunk.to_vec())
                        .collect())
                }
            }
            Some(_) => Err(Error::new(
                ErrorKind::MalformedTorrent,
                Cow::Borrowed("\"pieces\" does not map to a sequence of bytes."),
            )),
            None => Err(Error::new(
                ErrorKind::MalformedTorrent,
                Cow::Borrowed("\"pieces\" does not exist."),
            )),
        }
    }

    fn extract_extra_fields(dict: HashMap<String, BencodeElem>) -> Option<Dictionary> {
        if dict.is_empty() {
            None
        } else {
            Some(dict)
        }
    }
}

#[cfg(test)]
mod file_read_tests {
    use super::*;
    use std::iter::FromIterator;

    #[test]
    fn extract_file_ok() {
        let file = bencode_elem!({
            ("length", 42),
            ("path", ["root", ".bashrc"]),
            ("comment", "no comment"),
        });

        assert_eq!(
            File::extract_file(file).unwrap(),
            File {
                length: 42,
                path: PathBuf::from("root/.bashrc"),
                extra_fields: Some(HashMap::from_iter(
                    vec![("comment".to_string(), bencode_elem!("no comment"))].into_iter()
                )),
            }
        );
    }

    #[test]
    fn extract_file_not_dictionary() {
        let file = bencode_elem!([]);

        match File::extract_file(file) {
            Ok(_) => assert!(false),
            Err(e) => assert_eq!(e.kind(), ErrorKind::MalformedTorrent),
        }
    }

    #[test]
    fn extract_file_length_ok() {
        let mut dict =
            HashMap::from_iter(vec![("length".to_string(), bencode_elem!(42))].into_iter());
        assert_eq!(File::extract_file_length(&mut dict).unwrap(), 42);
    }

    #[test]
    fn extract_file_length_not_positive() {
        let mut dict =
            HashMap::from_iter(vec![("length".to_string(), bencode_elem!(0))].into_iter());

        match File::extract_file_length(&mut dict) {
            Ok(_) => assert!(false),
            Err(e) => assert_eq!(e.kind(), ErrorKind::MalformedTorrent),
        }
    }

    #[test]
    fn extract_file_length_not_integer() {
        let mut dict =
            HashMap::from_iter(vec![("length".to_string(), bencode_elem!("42"))].into_iter());

        match File::extract_file_length(&mut dict) {
            Ok(_) => assert!(false),
            Err(e) => assert_eq!(e.kind(), ErrorKind::MalformedTorrent),
        }
    }

    #[test]
    fn extract_file_length_missing() {
        let mut dict = HashMap::new();

        match File::extract_file_length(&mut dict) {
            Ok(_) => assert!(false),
            Err(e) => assert_eq!(e.kind(), ErrorKind::MalformedTorrent),
        }
    }

    #[test]
    fn extract_file_path_ok() {
        let mut dict = HashMap::from_iter(
            vec![("path".to_string(), bencode_elem!(["root", ".bashrc"]))].into_iter(),
        );

        assert_eq!(
            File::extract_file_path(&mut dict).unwrap(),
            PathBuf::from("root/.bashrc")
        );
    }

    #[test]
    fn extract_file_path_not_list() {
        let mut dict = HashMap::from_iter(
            vec![("path".to_string(), bencode_elem!("root/.bashrc"))].into_iter(),
        );

        match File::extract_file_path(&mut dict) {
            Ok(_) => assert!(false),
            Err(e) => assert_eq!(e.kind(), ErrorKind::MalformedTorrent),
        }
    }

    #[test]
    fn extract_file_path_missing() {
        let mut dict = HashMap::new();

        match File::extract_file_path(&mut dict) {
            Ok(_) => assert!(false),
            Err(e) => assert_eq!(e.kind(), ErrorKind::MalformedTorrent),
        }
    }

    #[test]
    fn extract_file_path_empty_list() {
        let mut dict =
            HashMap::from_iter(vec![("path".to_string(), bencode_elem!([]))].into_iter());

        match File::extract_file_path(&mut dict) {
            Ok(_) => assert!(false),
            Err(e) => assert_eq!(e.kind(), ErrorKind::MalformedTorrent),
        }
    }

    #[test]
    fn extract_file_path_component_not_string() {
        let mut dict = HashMap::from_iter(
            vec![
                (
                    "path".to_string(),
                    BencodeElem::List(vec![
                        BencodeElem::String("root".to_string()),
                        BencodeElem::Bytes(".bashrc".as_bytes().to_vec()),
                    ]),
                ),
            ].into_iter(),
        );

        match File::extract_file_path(&mut dict) {
            Ok(_) => assert!(false),
            Err(e) => assert_eq!(e.kind(), ErrorKind::MalformedTorrent),
        }
    }

    #[test]
    fn extract_file_path_component_invalid() {
        let mut dict = HashMap::from_iter(
            vec![
                (
                    "path".to_string(),
                    BencodeElem::List(vec![
                        BencodeElem::String("root".to_string()),
                        BencodeElem::String(".".to_string()),
                    ]),
                ),
            ].into_iter(),
        );

        match File::extract_file_path(&mut dict) {
            Ok(_) => assert!(false),
            Err(e) => assert_eq!(e.kind(), ErrorKind::MalformedTorrent),
        }
    }

    #[test]
    fn extract_file_path_component_invalid_2() {
        let mut dict = HashMap::from_iter(
            vec![
                (
                    "path".to_string(),
                    BencodeElem::List(vec![
                        BencodeElem::String("root".to_string()),
                        BencodeElem::String("..".to_string()),
                    ]),
                ),
            ].into_iter(),
        );

        match File::extract_file_path(&mut dict) {
            Ok(_) => assert!(false),
            Err(e) => assert_eq!(e.kind(), ErrorKind::MalformedTorrent),
        }
    }

    #[test]
    fn extract_file_extra_fields_ok() {
        assert_eq!(
            File::extract_file_extra_fields(HashMap::from_iter(
                vec![("comment".to_string(), bencode_elem!("none"))].into_iter()
            )),
            Some(HashMap::from_iter(
                vec![("comment".to_string(), bencode_elem!("none"))].into_iter()
            ))
        )
    }

    #[test]
    fn extract_file_extra_fields_none() {
        assert_eq!(File::extract_file_extra_fields(HashMap::new()), None)
    }
}

#[cfg(test)]
mod torrent_read_tests {
    // @note: `read_from_bytes()` and `read_from_file()` are not tested
    // as they are best left to integration tests (in `tests/`).
    use super::*;
    use std::iter::FromIterator;

    #[test]
    fn validate_ok() {
        // torrent is actually invalid (incorrect pieces' length)
        // keeping things simple for the sake of solely testing `validate()`
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
        };

        // use `clone()` here so we can test that `torrent` is not modified
        // accidentally by `validate()`
        assert_eq!(torrent.clone().validate().unwrap(), torrent);
    }

    #[test]
    fn validate_length_mismatch() {
        let torrent = Torrent {
            announce: "url".to_string(),
            announce_list: None,
            length: 6,
            files: None,
            name: "sample".to_string(),
            piece_length: 2,
            pieces: vec![vec![1, 2], vec![3, 4]],
            extra_fields: None,
            extra_info_fields: None,
        };

        match torrent.validate() {
            Ok(_) => assert!(false),
            Err(e) => assert_eq!(e.kind(), ErrorKind::MalformedTorrent),
        }
    }

    #[test]
    fn validate_length_not_positive() {
        let torrent = Torrent {
            announce: "url".to_string(),
            announce_list: None,
            length: 0,
            files: None,
            name: "sample".to_string(),
            piece_length: 2,
            pieces: vec![vec![1, 2], vec![3, 4]],
            extra_fields: None,
            extra_info_fields: None,
        };

        match torrent.validate() {
            Ok(_) => assert!(false),
            Err(e) => assert_eq!(e.kind(), ErrorKind::MalformedTorrent),
        }
    }

    #[test]
    fn validate_length_overflow() {
        let torrent = Torrent {
            announce: "url".to_string(),
            announce_list: None,
            length: 0,
            files: None,
            name: "sample".to_string(),
            piece_length: i64::max_value(),
            pieces: vec![vec![1, 2], vec![3, 4]],
            extra_fields: None,
            extra_info_fields: None,
        };

        match torrent.validate() {
            Ok(_) => assert!(false),
            Err(e) => assert_eq!(e.kind(), ErrorKind::MalformedTorrent),
        }
    }

    #[test]
    fn from_parsed_ok() {
        let dict = vec![
            bencode_elem!({
                ("announce", "url"),
                ("info", {
                    ("name", "??"),
                    ("length", 2),
                    ("piece length", 2),
                    (
                        "pieces",
                        (0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09,
                            0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10, 0x11, 0x12, 0x13)
                    ),
                }),
            }),
        ];

        assert_eq!(
            Torrent::from_parsed(dict).unwrap(),
            Torrent {
                announce: "url".to_string(),
                announce_list: None,
                length: 2,
                files: None,
                name: "??".to_string(),
                piece_length: 2,
                pieces: vec![
                    vec![
                        0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b,
                        0x0c, 0x0d, 0x0e, 0x0f, 0x10, 0x11, 0x12, 0x13,
                    ],
                ],
                extra_fields: None,
                extra_info_fields: None,
            }
        );
    }

    #[test]
    fn from_parsed_top_level_multiple_elem() {
        let dict = vec![bencode_elem!({}), bencode_elem!([])];

        match Torrent::from_parsed(dict) {
            Ok(_) => assert!(false),
            Err(e) => assert_eq!(e.kind(), ErrorKind::MalformedTorrent),
        }
    }

    #[test]
    fn from_parsed_top_level_no_elem() {
        let dict = Vec::new();

        match Torrent::from_parsed(dict) {
            Ok(_) => assert!(false),
            Err(e) => assert_eq!(e.kind(), ErrorKind::MalformedTorrent),
        }
    }

    #[test]
    fn from_parsed_top_level_not_dict() {
        let dict = vec![bencode_elem!([])];

        match Torrent::from_parsed(dict) {
            Ok(_) => assert!(false),
            Err(e) => assert_eq!(e.kind(), ErrorKind::MalformedTorrent),
        }
    }

    #[test]
    fn from_parsed_info_missing() {
        // "announce" is needed here because it is parsed before "info"
        // missing "announce-list" is fine as that won't trigger an error
        let dict = vec![bencode_elem!({ ("announce", "url") })];

        match Torrent::from_parsed(dict) {
            Ok(_) => assert!(false),
            Err(e) => assert_eq!(e.kind(), ErrorKind::MalformedTorrent),
        }
    }

    #[test]
    fn from_parsed_info_not_dict() {
        // "announce" is needed here because it is parsed before "info"
        // missing "announce-list" is fine as that won't trigger an error
        let parsed = vec![bencode_elem!({ ("announce", "url"), ("info", []) })];

        match Torrent::from_parsed(parsed) {
            Ok(_) => assert!(false),
            Err(e) => assert_eq!(e.kind(), ErrorKind::MalformedTorrent),
        }
    }

    #[test]
    fn extract_announce_ok() {
        let mut dict =
            HashMap::from_iter(vec![("announce".to_string(), bencode_elem!("url"))].into_iter());

        assert_eq!(
            Torrent::extract_announce(&mut dict).unwrap(),
            "url".to_string()
        );
    }

    #[test]
    fn extract_announce_missing() {
        let mut dict = HashMap::new();

        match Torrent::extract_announce(&mut dict) {
            Ok(_) => assert!(false),
            Err(e) => assert_eq!(e.kind(), ErrorKind::MalformedTorrent),
        }
    }

    #[test]
    fn extract_announce_not_string() {
        let mut dict = HashMap::from_iter(
            vec![
                (
                    "announce".to_string(),
                    BencodeElem::Bytes("url".as_bytes().to_vec()),
                ),
            ].into_iter(),
        );

        match Torrent::extract_announce(&mut dict) {
            Ok(_) => assert!(false),
            Err(e) => assert_eq!(e.kind(), ErrorKind::MalformedTorrent),
        }
    }

    #[test]
    fn extract_announce_list_tier_ok() {
        let tier = bencode_elem!(["url1", "url2"]);

        assert_eq!(
            Torrent::extract_announce_list_tier(tier).unwrap(),
            vec!["url1".to_string(), "url2".to_string()]
        );
    }

    #[test]
    fn extract_announce_list_tier_not_list() {
        let tier = bencode_elem!({});
        match Torrent::extract_announce_list_tier(tier) {
            Ok(_) => assert!(false),
            Err(e) => assert_eq!(e.kind(), ErrorKind::MalformedTorrent),
        }
    }

    #[test]
    fn extract_announce_list_tier_url_not_string() {
        let tier = BencodeElem::List(vec![
            bencode_elem!("url1"),
            BencodeElem::Bytes("url2".as_bytes().to_vec()),
        ]);

        match Torrent::extract_announce_list_tier(tier) {
            Ok(_) => assert!(false),
            Err(e) => assert_eq!(e.kind(), ErrorKind::MalformedTorrent),
        }
    }

    #[test]
    fn extract_announce_list_ok() {
        let mut dict = HashMap::from_iter(
            vec![
                (
                    "announce-list".to_string(),
                    bencode_elem!([["url1", "url2"], ["url3", "url4"]]),
                ),
            ].into_iter(),
        );

        assert_eq!(
            Torrent::extract_announce_list(&mut dict).unwrap(),
            Some(vec![
                vec!["url1".to_string(), "url2".to_string()],
                vec!["url3".to_string(), "url4".to_string()],
            ])
        );
    }

    #[test]
    fn extract_announce_list_missing() {
        let mut dict = HashMap::new();
        assert_eq!(Torrent::extract_announce_list(&mut dict).unwrap(), None);
    }

    #[test]
    fn extract_announce_list_not_list() {
        let mut dict = HashMap::from_iter(vec![("announce-list".to_string(), bencode_elem!({}))]);

        match Torrent::extract_announce_list(&mut dict) {
            Ok(_) => assert!(false),
            Err(e) => assert_eq!(e.kind(), ErrorKind::MalformedTorrent),
        }
    }

    #[test]
    fn extract_files_ok() {
        let mut dict = HashMap::from_iter(
            vec![
                (
                    "files".to_string(),
                    bencode_elem!([{
                        ("length", 42),
                        ("path", ["root", ".bashrc"]),
                        ("comment", "no comment"),
                    }]),
                ),
            ].into_iter(),
        );

        let files = Torrent::extract_files(&mut dict).unwrap().unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(
            files[0],
            File {
                length: 42,
                path: PathBuf::from("root/.bashrc"),
                extra_fields: Some(HashMap::from_iter(
                    vec![("comment".to_string(), bencode_elem!("no comment"))].into_iter()
                )),
            }
        );
    }

    #[test]
    fn extract_files_not_list() {
        let mut dict = HashMap::from_iter(vec![("files".to_string(), bencode_elem!({}))]);

        match Torrent::extract_files(&mut dict) {
            Ok(_) => assert!(false),
            Err(e) => assert_eq!(e.kind(), ErrorKind::MalformedTorrent),
        }
    }

    #[test]
    fn extract_files_missing() {
        let mut dict = HashMap::new();
        assert_eq!(Torrent::extract_files(&mut dict).unwrap(), None);
    }

    #[test]
    fn extract_files_empty_list() {
        let mut dict = HashMap::from_iter(vec![("files".to_string(), bencode_elem!([]))]);

        match Torrent::extract_files(&mut dict) {
            Ok(_) => assert!(false),
            Err(e) => assert_eq!(e.kind(), ErrorKind::MalformedTorrent),
        }
    }

    #[test]
    fn extract_length_ok() {
        let mut dict =
            HashMap::from_iter(vec![("length".to_string(), bencode_elem!(42))].into_iter());
        assert_eq!(Torrent::extract_length(&mut dict, &None).unwrap(), 42);
    }

    #[test]
    fn extract_length_conflict_with_files() {
        let mut dict =
            HashMap::from_iter(vec![("length".to_string(), bencode_elem!(42))].into_iter());
        let files = Some(vec![
            File {
                length: 100,
                path: PathBuf::new(),
                extra_fields: None,
            },
        ]);

        match Torrent::extract_length(&mut dict, &files) {
            Ok(_) => assert!(false),
            Err(e) => assert_eq!(e.kind(), ErrorKind::MalformedTorrent),
        }
    }

    #[test]
    fn extract_length_not_integer() {
        let mut dict =
            HashMap::from_iter(vec![("length".to_string(), bencode_elem!("42"))].into_iter());

        match Torrent::extract_length(&mut dict, &None) {
            Ok(_) => assert!(false),
            Err(e) => assert_eq!(e.kind(), ErrorKind::MalformedTorrent),
        }
    }

    #[test]
    fn extract_length_missing_no_files() {
        let mut dict = HashMap::new();

        match Torrent::extract_length(&mut dict, &None) {
            Ok(_) => assert!(false),
            Err(e) => assert_eq!(e.kind(), ErrorKind::MalformedTorrent),
        }
    }

    #[test]
    fn extract_length_missing_have_files() {
        let mut dict = HashMap::new();
        let files = Some(vec![
            File {
                length: 100,
                path: PathBuf::new(),
                extra_fields: None,
            },
        ]);

        assert_eq!(Torrent::extract_length(&mut dict, &files).unwrap(), 100);
    }

    #[test]
    fn extract_length_missing_have_files_overflow() {
        let mut dict = HashMap::new();
        let files = Some(vec![
            File {
                length: 1,
                path: PathBuf::new(),
                extra_fields: None,
            },
            File {
                length: i64::max_value(),
                path: PathBuf::new(),
                extra_fields: None,
            },
        ]);

        match Torrent::extract_length(&mut dict, &files) {
            Ok(_) => assert!(false),
            Err(e) => assert_eq!(e.kind(), ErrorKind::MalformedTorrent),
        }
    }

    #[test]
    fn extract_name_ok() {
        let mut dict =
            HashMap::from_iter(vec![("name".to_string(), bencode_elem!("not name"))].into_iter());

        assert_eq!(
            Torrent::extract_name(&mut dict).unwrap(),
            "not name".to_string()
        );
    }

    #[test]
    fn extract_name_not_string() {
        let mut dict = HashMap::from_iter(
            vec![
                (
                    "name".to_string(),
                    BencodeElem::Bytes("not name".as_bytes().to_vec()),
                ),
            ].into_iter(),
        );

        match Torrent::extract_name(&mut dict) {
            Ok(_) => assert!(false),
            Err(e) => assert_eq!(e.kind(), ErrorKind::MalformedTorrent),
        }
    }

    #[test]
    fn extract_name_missing() {
        let mut dict = HashMap::new();

        match Torrent::extract_name(&mut dict) {
            Ok(_) => assert!(false),
            Err(e) => assert_eq!(e.kind(), ErrorKind::MalformedTorrent),
        }
    }

    #[test]
    fn extract_piece_length_ok() {
        let mut dict =
            HashMap::from_iter(vec![("piece length".to_string(), bencode_elem!(1))].into_iter());
        assert_eq!(Torrent::extract_piece_length(&mut dict).unwrap(), 1);
    }

    #[test]
    fn extract_piece_length_not_integer() {
        let mut dict =
            HashMap::from_iter(vec![("piece length".to_string(), bencode_elem!("1"))].into_iter());

        match Torrent::extract_piece_length(&mut dict) {
            Ok(_) => assert!(false),
            Err(e) => assert_eq!(e.kind(), ErrorKind::MalformedTorrent),
        }
    }

    #[test]
    fn extract_piece_length_missing() {
        let mut dict = HashMap::new();

        match Torrent::extract_piece_length(&mut dict) {
            Ok(_) => assert!(false),
            Err(e) => assert_eq!(e.kind(), ErrorKind::MalformedTorrent),
        }
    }

    #[test]
    fn extract_piece_length_not_positive() {
        let mut dict =
            HashMap::from_iter(vec![("piece length".to_string(), bencode_elem!(0))].into_iter());

        match Torrent::extract_piece_length(&mut dict) {
            Ok(_) => assert!(false),
            Err(e) => assert_eq!(e.kind(), ErrorKind::MalformedTorrent),
        }
    }

    #[test]
    fn extract_pieces_ok() {
        let mut dict = HashMap::from_iter(
            vec![
                (
                    "pieces".to_string(),
                    BencodeElem::Bytes(vec![
                        0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b,
                        0x0c, 0x0d, 0x0e, 0x0f, 0x10, 0x11, 0x12, 0x13,
                    ]),
                ),
            ].into_iter(),
        );

        let pieces = Torrent::extract_pieces(&mut dict).unwrap();
        assert_eq!(pieces.len(), 1);
        assert_eq!(pieces[0].len(), PIECE_STRING_LENGTH);
        assert_eq!(
            pieces[0],
            vec![
                0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d,
                0x0e, 0x0f, 0x10, 0x11, 0x12, 0x13,
            ]
        );
    }

    #[test]
    fn extract_pieces_not_bytes() {
        let mut dict =
            HashMap::from_iter(vec![("pieces".to_string(), bencode_elem!("???"))].into_iter());

        match Torrent::extract_pieces(&mut dict) {
            Ok(_) => assert!(false),
            Err(e) => assert_eq!(e.kind(), ErrorKind::MalformedTorrent),
        }
    }

    #[test]
    fn extract_pieces_missing() {
        let mut dict = HashMap::new();

        match Torrent::extract_pieces(&mut dict) {
            Ok(_) => assert!(false),
            Err(e) => assert_eq!(e.kind(), ErrorKind::MalformedTorrent),
        }
    }

    #[test]
    fn extract_pieces_empty() {
        let mut dict =
            HashMap::from_iter(vec![("pieces".to_string(), bencode_elem!(()))].into_iter());

        match Torrent::extract_pieces(&mut dict) {
            Ok(_) => assert!(false),
            Err(e) => assert_eq!(e.kind(), ErrorKind::MalformedTorrent),
        }
    }

    #[test]
    fn extract_pieces_invalid_length() {
        let mut dict = HashMap::from_iter(
            vec![
                (
                    "pieces".to_string(),
                    BencodeElem::Bytes(vec![
                        0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b,
                        0x0c, 0x0d, 0x0e, 0x0f, 0x10, 0x11, 0x12,
                    ]),
                ),
            ].into_iter(),
        );

        match Torrent::extract_pieces(&mut dict) {
            Ok(_) => assert!(false),
            Err(e) => assert_eq!(e.kind(), ErrorKind::MalformedTorrent),
        }
    }

    #[test]
    fn extract_extra_fields_ok() {
        assert_eq!(
            Torrent::extract_extra_fields(HashMap::from_iter(
                vec![("comment".to_string(), bencode_elem!("none"))].into_iter()
            )),
            Some(HashMap::from_iter(
                vec![("comment".to_string(), bencode_elem!("none"))].into_iter()
            ))
        )
    }

    #[test]
    fn extract_extra_fields_none() {
        assert_eq!(Torrent::extract_extra_fields(HashMap::new()), None)
    }
}
