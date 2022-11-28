use std::{fs::File, io::Read};

use rust_image_decoder::{
    image::{ImageDecoder, ImageEncoder},
    jpeg::JPEGDecoder,
    ppm::PPMEncoder,
};

fn main() {
    let buffer = {
        let filename = "image-decoder-app/resources/test2.jpg";
        let mut f = File::open(&filename).expect("no file found");
        let metadata = std::fs::metadata(&filename).expect("unable to read metadata");
        let mut buffer = vec![0; metadata.len() as usize];
        f.read(&mut buffer).expect("buffer overflow");
        buffer
    };

    let jpeg = JPEGDecoder::new(buffer.as_slice());
    let bitmap = jpeg.decode().expect("Failed to read JPEG image");
    let ppm_encoder = PPMEncoder::new(&bitmap);
    ppm_encoder
        .encode_to_file("test.ppm")
        .expect("Failed to write PPM result to file");
}
