use crate::error::Result;

/// Stores a single frame of image data in a simple bitmap form
#[derive(Debug, Default)]
pub struct Bitmap {
    /// The number of color channels in the image. Ex. RGBA = 4
    pub channels: u8,

    /// The size of the image
    pub size: (u16, u16),
    /// The raw bitmap data
    pub data: Vec<u8>,
}

/// Used to decode an image. This trait can be implemented for any image format I want to decode.
pub trait ImageDecoder<'data> {
    /// Supplies the decode with the image data
    fn new(image_data: &'data [u8]) -> Self;
    /// Decodes the image
    fn decode(&self) -> Result<Bitmap>;
}

/// Used to encode an image. This trait can be implemented for any image format I want to encode.
pub trait ImageEncoder<'bitmap> {
    /// Supplies the encoder with a raw bitmap to encode.
    fn new(bitmap: &'bitmap Bitmap) -> Self;
    /// Encodes the bitmap and saves the result to a file at the given path.
    fn encode_to_file(&self, path: &str) -> std::io::Result<()>;
}
