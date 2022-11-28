use std::{
    fs::File,
    io::{self, Write},
};

use crate::image::{Bitmap, ImageEncoder};

/// PPM encoder
pub struct PPMEncoder<'bitmap> {
    bitmap: &'bitmap Bitmap,
}

impl<'bitmap> ImageEncoder<'bitmap> for PPMEncoder<'bitmap> {
    fn new(bitmap: &'bitmap Bitmap) -> Self {
        Self { bitmap }
    }

    fn encode_to_file(&self, path: &str) -> io::Result<()> {
        let mut file = File::create(path).expect("Failed to create file");
        file.write(format!("P{}\n", self.bitmap.channels).as_bytes())
            .expect("Failed to write file");
        file.write(format!("{} {}\n", self.bitmap.size.0, self.bitmap.size.1).as_bytes())
            .expect("Failed to write file");
        file.write(format!("255\n").as_bytes())
            .expect("Failed to write file");

        for y in 0..self.bitmap.size.1 {
            for x in 0..self.bitmap.size.0 {
                let index = ((y as usize * self.bitmap.size.0 as usize) + x as usize)
                    * self.bitmap.channels as usize;
                file.write(
                    format!(
                        "{} {} {}\n",
                        self.bitmap.data[index + 0],
                        self.bitmap.data[index + 1],
                        self.bitmap.data[index + 2]
                    )
                    .as_bytes(),
                )
                .expect("Failed to write file");
            }
        }
        Ok(())
    }
}
