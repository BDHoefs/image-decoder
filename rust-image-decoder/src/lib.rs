#![warn(missing_docs)]

//! Allows opening and processing of various(just JPEG for now) image files.
mod bitstream;
mod error;
/// Defines types for decoding images
pub mod image;
/// Decoder for JPEG images
pub mod jpeg;
/// Encoder for PPM images
pub mod ppm;
