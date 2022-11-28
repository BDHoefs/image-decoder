use std::f32::consts::PI;

use crate::{
    bitstream::Bitstream,
    error::Result,
    image::Bitmap,
    jpeg::jpeg_reader::{JPEGMarker, JPEGParser},
};
use crate::{error::Error, jpeg::header::*};

#[rustfmt::skip]
pub const ZIGZAG_MAP: &'static [(u8, u8)] = 
    &[(0, 0), (0, 1), (1, 0), (2, 0), (1, 1), (0, 2), (0, 3), (1, 2),
          (2, 1), (3, 0), (4, 0), (3, 1), (2, 2), (1, 3), (0, 4), (0, 5),
          (1, 4), (2, 3), (3, 2), (4, 1), (5, 0), (6, 0), (5, 1), (4, 2),
          (3, 3), (2, 4), (1, 5), (0, 6), (0, 7), (1, 6), (2, 5), (3, 4),
          (4, 3), (5, 2), (6, 1), (7, 0), (7, 1), (6, 2), (5, 3), (4, 4),
          (3, 5), (2, 6), (1, 7), (2, 7), (3, 6), (4, 5), (5, 4), (6, 3),
          (7, 2), (7, 3), (6, 4), (5, 5), (4, 6), (3, 7), (4, 7), (5, 6),
          (6, 5), (7, 4), (7, 5), (6, 6), (5, 7), (6, 7), (7, 6), (7, 7)];

pub struct JPEGDecoder<'data> {
    reader: JPEGParser<'data>,
    dc_predictions: Vec<i16>,
}

impl<'data> JPEGDecoder<'data> {
    pub fn new(data: &'data [u8]) -> Self {
        Self {
            reader: JPEGParser::new(data),
            dc_predictions: vec![],
        }
    }

    pub fn parse(&mut self) -> Result<HeaderInfo> {
        HeaderInfo::read_header_info(&mut self.reader)
    }

    pub fn read_scan(&mut self, header: &HeaderInfo) -> Result<Bitmap> {
        let huffman_data = self.read_huffman_data()?;
        let mut bitstream = Bitstream::new(&huffman_data.as_slice());
        self.dc_predictions = vec![0; header.scan_info.components.len() + 1];

        let mut blocks = vec![
            vec![
                Macroblock::new(header.mcu_info.max_xy_sampling_factor);
                header.mcu_info.mcu_padded_dimensions.0 as usize
            ];
            header.mcu_info.mcu_padded_dimensions.1 as usize
        ];

        for vert in 0..header.mcu_info.mcu_padded_dimensions.1 {
            for horiz in 0..header.mcu_info.mcu_padded_dimensions.0 {
                blocks[vert as usize][horiz as usize] =
                    self.decode_block(&mut bitstream, header)?;
            }
        }

        Ok(Self::blocks_to_bitmap(&mut blocks, header))
    }

    fn blocks_to_bitmap(blocks: &mut Vec<Vec<Macroblock>>, header: &HeaderInfo) -> Bitmap {
        let channels = header.components.len() as u8;
        let size = header.frame_info.image_size;
        let mut data = vec![0u8; size.0 as usize * size.1 as usize * channels as usize];
        for y in 0..size.1 {
            for x in 0..size.0 {
                let block_y = y / (8 * header.mcu_info.max_xy_sampling_factor.1 as u16);
                let block_x = x / (8 * header.mcu_info.max_xy_sampling_factor.0 as u16);
                let pixel_y = y % (8 * header.mcu_info.max_xy_sampling_factor.1 as u16);
                let pixel_x = x % (8 * header.mcu_info.max_xy_sampling_factor.0 as u16);

                let block = &mut blocks[block_y as usize][block_x as usize];
                // TODO: Support greyscale
                let y_cb_cr = (
                    block.get_component(1)[pixel_y as usize][pixel_x as usize],
                    block.get_component(2)[pixel_y as usize][pixel_x as usize],
                    block.get_component(3)[pixel_y as usize][pixel_x as usize],
                );

                let rgb = Self::ycbcr_to_rgb(y_cb_cr);

                let data_index = ((y as usize * size.0 as usize) + x as usize) * channels as usize;
                data[data_index + 0] = rgb.0;
                data[data_index + 1] = rgb.1;
                data[data_index + 2] = rgb.2;
            }
        }
        Bitmap {
            channels,
            size,
            data,
        }
    }

    fn ycbcr_to_rgb(y_cb_cr: (i16, i16, i16)) -> (u8, u8, u8) {
        let lum = y_cb_cr.0 as f32;
        let cb = y_cb_cr.1 as f32;
        let cr = y_cb_cr.2 as f32;

        let red = (cr * (2f32 - 2f32 * 0.299)) + lum;
        let blue = (cb * (2f32 - 2f32 * 0.114)) + lum;
        let green = (lum - (0.114 * blue) - (0.299 * red)) / 0.587;

        (
            (red + 128f32) as u8,
            (green + 128f32) as u8,
            (blue + 128f32) as u8,
        )
    }

    fn decode_block(
        &mut self,
        bitstream: &mut Bitstream,
        header: &HeaderInfo,
    ) -> Result<Macroblock> {
        let mut block = Macroblock::new(header.mcu_info.max_xy_sampling_factor);

        // Decode each MCU
        for component in &header.components {
            let dc_table = header.dc_huff_tables.get(&component.scan.dc_table).unwrap();
            let ac_table = header.ac_huff_tables.get(&component.scan.ac_table).unwrap();
            let qtable = header
                .quant_tables
                .get(&component.frame.qtable_id)
                .unwrap()
                .table;

            let component_block = block.get_component(component.scan.selector);

            for mcu_row in 0..component.frame.xy_sampling_factor.1 {
                for mcu_col in 0..component.frame.xy_sampling_factor.0 {
                    let base_y = mcu_row as usize * 8;
                    let base_x = mcu_col as usize * 8;

                    let mut dct_coefficients = vec![0i16; 64];

                    // Calculate DC coefficient
                    // https://www.w3.org/Graphics/JPEG/itu-t81.pdf
                    // F.2.2.1 Page 104
                    let (dc_code, _) = self.decode_next_value(bitstream, dc_table)?; // DECODE
                    let mut diff = bitstream.read_bits(dc_code as usize)? as i16; // RECEIVE

                    if dc_code != 0 && diff < (1 << (dc_code - 1)) {
                        diff -= (1 << dc_code) - 1; // EXTEND, If MSB is 0 then negative. 1 is positive
                    }

                    let dc_coefficient =
                        self.dc_predictions[component.scan.selector as usize] + diff;

                    self.dc_predictions[component.scan.selector as usize] = dc_coefficient;

                    dct_coefficients[0] = dc_coefficient;

                    // Calculate AC coefficients
                    // https://www.w3.org/Graphics/JPEG/itu-t81.pdf
                    // F.13 Page 106

                    let mut k = 0;
                    while k != 63 {
                        k += 1;

                        let (huffman_val, _) = self.decode_next_value(bitstream, ac_table)?;

                        match huffman_val {
                            0x00 => {
                                break;
                            }
                            0xF0 => {
                                k += 15; // Skip 15+1(top of loop) zeroes.
                                continue;
                            }
                            _ => {
                                let run_length = huffman_val >> 4;
                                k += run_length;

                                if k > 64 {
                                    return Err(Error::Malformed("Run length exceeds max K of 64"));
                                }

                                let code_length = huffman_val & 0b1111;
                                let mut value = bitstream.read_bits(code_length as usize)? as i16;

                                // EXTEND
                                if value < (1 << (code_length - 1)) {
                                    value -= (1 << code_length) - 1;
                                }

                                dct_coefficients[k as usize] = value;
                            }
                        }
                    }

                    // Dequantize and unzigzag
                    for i in 0..64 {
                        let (row, col) = ZIGZAG_MAP[i];
                        component_block[row as usize + base_y][col as usize + base_x] =
                            dct_coefficients[i] * qtable[row as usize][col as usize] as i16;
                    }

                    // Perform the IDCT
                    // https://www.w3.org/Graphics/JPEG/itu-t81.pdf
                    // A.3.3 Page 27
                    let mut idct_block = component_block.clone();
                    for y in 0..8 {
                        for x in 0..8 {
                            let mut value = 0.0f32;
                            for u in 0..8 {
                                for v in 0..8 {
                                    let cu = if u == 0 {
                                        1f32 / f32::sqrt(2.0f32)
                                    } else {
                                        1.0f32
                                    };
                                    let cv = if v == 0 {
                                        1f32 / f32::sqrt(2.0f32)
                                    } else {
                                        1f32
                                    };
                                    let idct_val = cu as f32
                                        * cv as f32
                                        * f32::cos(
                                            ((2.0f32 * x as f32 + 1.0f32) * u as f32 * PI)
                                                / 16.0f32,
                                        )
                                        * f32::cos(
                                            ((2.0f32 * y as f32 + 1.0f32) * v as f32 * PI)
                                                / 16.0f32,
                                        );

                                    let coeff = component_block[base_y + v][base_x + u] as f32;
                                    value += idct_val * coeff;
                                }
                            }

                            value /= 4.0f32;

                            idct_block[base_y + y][base_x + x] = value as i16;
                        }
                    }

                    *component_block = idct_block;
                }
            }

            // Stretch subsampled components to the correct size
            let horiz_ratio =
                header.mcu_info.max_xy_sampling_factor.0 / component.frame.xy_sampling_factor.0;
            let vert_ratio =
                header.mcu_info.max_xy_sampling_factor.1 / component.frame.xy_sampling_factor.1;

            if horiz_ratio > 1 || vert_ratio > 1 {
                let mut stretched_block = component_block.clone();
                for y in 0..(8 * header.mcu_info.max_xy_sampling_factor.1) {
                    for x in 0..(8 * header.mcu_info.max_xy_sampling_factor.0) {
                        let source_y = y as usize / vert_ratio as usize;
                        let source_x = x as usize / horiz_ratio as usize;

                        stretched_block[y as usize][x as usize] =
                            component_block[source_y][source_x];
                    }
                }
                *component_block = stretched_block;
            }
        }
        Ok(block)
    }

    fn decode_next_value(
        &mut self,
        bitstream: &mut Bitstream,
        table: &HuffmanTable,
    ) -> Result<(u8, u8)> {
        let mut code: i32 = 0;
        let mut code_cursor: usize = 0;

        for i in 0u8..16 {
            let bit = bitstream.read_bits(1)? as i32;
            code = (code << 1) | bit;
            for _ in 0..table.bitcode_counts[i as usize] {
                if code == table.codes[code_cursor] as i32 {
                    return Ok((table.symbols[code_cursor], i));
                }
                code_cursor += 1;
            }
        }

        Err(Error::UnsupportedFeature(
            "JPEG has code longer than the 16 bit maximum for baseline JPEGs.",
        ))
    }

    fn read_huffman_data(&mut self) -> Result<Vec<u8>> {
        let mut huffman_data: Vec<u8> = vec![];
        let mut current_byte = self.reader.read_next_byte()?;

        loop {
            let last_byte = current_byte;
            current_byte = self.reader.read_next_byte()?;

            if last_byte == 0xFF {
                if current_byte == 0x00 {
                    current_byte = self.reader.read_next_byte()?;
                    huffman_data.push(last_byte);
                    continue;
                }

                let marker_data = 0xFF00 | current_byte as u16;
                let marker = JPEGParser::to_marker(marker_data)?;

                if marker == JPEGMarker::EOI {
                    return Ok(huffman_data);
                }
            } else {
                huffman_data.push(last_byte);
            }
        }
    }
}

#[derive(Debug, Clone)]
struct Macroblock {
    y: Vec<Vec<i16>>,
    cb: Vec<Vec<i16>>,
    cr: Vec<Vec<i16>>,
}

impl Macroblock {
    pub fn new(block_sample_size: (u8, u8)) -> Self {
        Self {
            y: vec![vec![0; 8 * block_sample_size.0 as usize]; 8 * block_sample_size.1 as usize],
            cb: vec![vec![0; 8 * block_sample_size.0 as usize]; 8 * block_sample_size.1 as usize],
            cr: vec![vec![0; 8 * block_sample_size.0 as usize]; 8 * block_sample_size.1 as usize],
        }
    }
    pub fn get_component(&mut self, selector: u8) -> &mut Vec<Vec<i16>> {
        match selector {
            1 => &mut self.y,
            2 => &mut self.cb,
            3 => &mut self.cr,
            _ => panic!("Invalid component selector"),
        }
    }
}
