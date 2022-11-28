use byteorder::{BigEndian, ReadBytesExt};
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;
use std::io::{Cursor, Seek, SeekFrom};

use crate::error::{Error, Result};

#[allow(non_camel_case_types)]
#[derive(PartialEq, PartialOrd, FromPrimitive, Debug, Clone, Copy)]
pub enum JPEGMarker {
    APP0 = 0xFFE0,
    APP1 = 0xFFE1,
    APP2 = 0xFFE2,
    APP3 = 0xFFE3,
    APP4 = 0xFFE4,
    APP5 = 0xFFE5,
    APP6 = 0xFFE6,
    APP7 = 0xFFE7,
    APP8 = 0xFFE8,
    APP9 = 0xFFE9,
    APP10 = 0xFFEA,
    APP11 = 0xFFEB,
    APP12 = 0xFFEC,
    APP13 = 0xFFED,
    APP14 = 0xFFEE,
    APP15 = 0xFFEF,

    RESERVED1 = 0xFF1,
    RESERVED2 = 0xFF2,
    RESERVED3 = 0xFF3,
    RESERVED4 = 0xFF4,
    RESERVED5 = 0xFF5,
    RESERVED6 = 0xFF6,
    RESERVED7 = 0xFF7,
    RESERVED8 = 0xFF8,
    RESERVED9 = 0xFF9,
    RESERVED10 = 0xFFA,
    RESERVED11 = 0xFFB,
    RESERVED12 = 0xFFC,
    RESERVED13 = 0xFFD,

    RST0 = 0xFFD0,
    RST1 = 0xFFD1,
    RST2 = 0xFFD2,
    RST3 = 0xFFD3,
    RST4 = 0xFFD4,
    RST5 = 0xFFD5,
    RST6 = 0xFFD6,
    RST7 = 0xFFD7,

    DHP = 0xFFDE,
    EXP = 0xFFDF,

    DHT = 0xFFC4,
    DQT = 0xFFDB,
    EOI = 0xFFD9,
    RST = 0xFFDD,
    SOF0 = 0xFFC0, // Only support baseline DCT for now, I may add progressive later.
    SOI = 0xFFD8,
    SOS = 0xFFDA,
    COM = 0xFFFE,
}

pub struct JPEGParser<'data> {
    cursor: Cursor<&'data [u8]>,
}

impl<'data> JPEGParser<'data> {
    pub fn new(data: &'data [u8]) -> Self {
        Self {
            cursor: Cursor::new(data),
        }
    }

    pub fn to_marker(word: u16) -> Result<JPEGMarker> {
        if let Some(marker) = FromPrimitive::from_u16(word) {
            // Unfortunately these can't be matched as ranges even though they're PartialOrd, only numbers support ranges.
            if marker >= JPEGMarker::RESERVED1 && marker <= JPEGMarker::RESERVED13 {
                return Ok(marker);
            }
            if marker >= JPEGMarker::RST0 && marker <= JPEGMarker::RST7 {
                return Ok(marker);
            }
            if marker >= JPEGMarker::APP0 && marker <= JPEGMarker::APP15 {
                return Ok(marker);
            }
            match marker {
                JPEGMarker::COM
                | JPEGMarker::DHP
                | JPEGMarker::EXP
                | JPEGMarker::EOI
                | JPEGMarker::DHT
                | JPEGMarker::DQT
                | JPEGMarker::RST
                | JPEGMarker::SOF0 // Only support baseline DCT for now
                | JPEGMarker::SOI
                | JPEGMarker::SOS => return Ok(marker),
                _ => {}
            };

            return Err(Error::Malformed(
                "Marker not supported. Newly added marker may need to be implemented.",
            ));
        } else {
            return Err(Error::Malformed("Marker not supported"));
        }
    }

    pub fn read_next_word(&mut self) -> Result<u16> {
        match self.cursor.read_u16::<BigEndian>() {
            Ok(val) => return Ok(val),
            Err(_) => return Err(Error::Malformed("Unexpected end of input")),
        }
    }

    pub fn read_next_byte(&mut self) -> Result<u8> {
        match self.cursor.read_u8() {
            Ok(val) => return Ok(val),
            Err(_) => return Err(Error::Malformed("Unexpected end of input")),
        }
    }

    pub fn read_next_marker(&mut self) -> Result<JPEGMarker> {
        let word = self.read_next_word()?;
        match Self::to_marker(word) {
            Err(_) => {
                // Try to find another valid marker later in the stream
                if word != 0xFFFF {
                    return Err(Error::Malformed("Invalid JPEG file"));
                }

                loop {
                    let next = self.read_next_byte()?;
                    if next == 0x00 {
                        return Err(Error::Malformed("Invalid JPEG file"));
                    }

                    if word == 0xFF {
                        let next = 0xFF00 | next as u16;
                        let result = Self::to_marker(next);
                        if let Ok(marker) = result {
                            return Ok(marker);
                        } else if let Err(msg) = result {
                            return Err(msg);
                        }
                    }
                }
            }
            Ok(marker) => return Ok(marker),
        };
    }

    pub fn skip_marker_with_length(&mut self) -> Result<()> {
        let byte_length = self.read_next_word()? - 2;
        if let Ok(_) = self.cursor.seek(SeekFrom::Current(byte_length as i64)) {
            Ok(())
        } else {
            Err(Error::Malformed("JPEG marker with length contained a length longer than the remaining size of the JPEG file"))
        }
    }

    pub fn position(&self) -> u64 {
        self.cursor.position()
    }
}

#[rustfmt::skip]
#[allow(dead_code)]
static TEST_HEADER: [u8; 28] = [
    0xFF, 0xD8, // Start of image
    0xFF, 0xC0, // Start of frame
    0, 17,          // Length
    8,              // Precision
    0, 128,         // Height
    0, 128,         // Width
    3,              // Component count
    0, 0, 0,
    0, 0, 0,
    0, 0, 0,        // Component data
    0xFF, 0xFE, // Commment
    0, 3,           // Length
    65,             // Content
    0xFF, 0xD8  // Invalid marker
];

#[test]
fn read_words() {
    let mut reader = JPEGParser::new(&TEST_HEADER);
    assert_eq!(reader.read_next_byte().unwrap(), 0xFF);
    assert_eq!(reader.read_next_byte().unwrap(), 0xD8);

    assert_eq!(reader.read_next_word().unwrap(), 0xFFC0);
}

#[test]
fn read_markers() {
    let mut reader = JPEGParser::new(&TEST_HEADER);
    assert_eq!(reader.read_next_marker().unwrap(), JPEGMarker::SOI);
    assert_eq!(reader.read_next_marker().unwrap(), JPEGMarker::SOF0);
    assert!(!reader.skip_marker_with_length().is_err());
    assert_eq!(reader.read_next_marker().unwrap(), JPEGMarker::COM);
}
