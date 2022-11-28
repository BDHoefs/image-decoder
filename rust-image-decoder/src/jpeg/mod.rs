mod header;
mod jpeg_core;
mod jpeg_reader;

use crate::{
    error::Result,
    image::{Bitmap, ImageDecoder},
};

/// Contains JPEG image data
pub struct JPEGDecoder<'data> {
    image_data: &'data [u8],
}

impl<'data> ImageDecoder<'data> for JPEGDecoder<'data> {
    /// Initializes the JPEG decoder from a byte slice
    fn new(image_data: &'data [u8]) -> Self {
        Self { image_data }
    }

    fn decode(&self) -> Result<Bitmap> {
        let mut decoder = jpeg_core::JPEGDecoder::new(self.image_data);
        let header = decoder.parse()?;
        decoder.read_scan(&header)
    }
}
