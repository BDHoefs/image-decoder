pub type Result<T> = core::result::Result<T, Error>;

/// Describes an error encountered while reading an image.
#[derive(Debug)]
pub enum Error {
    /// The image is malformed in some way. The string describes how.
    Malformed(&'static str),
    /// A feature is not supported by the decoder
    UnsupportedFeature(&'static str),
    /// The decoder had a problem
    InternalError(&'static str),
    /// There was an error reading the image
    Io(std::io::Error),
}
