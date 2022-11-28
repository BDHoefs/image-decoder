use std::{cmp::max, collections::HashMap};

use crate::{
    error::{Error, Result},
    jpeg::jpeg_reader::*,
};

use super::jpeg_core::ZIGZAG_MAP;

#[derive(Debug, Default)]
pub enum HuffmanTableType {
    #[default]
    Ac,
    Dc,
}

/// Defines a JPEG huffman table
#[derive(Debug, Default)]
pub struct HuffmanTable {
    pub table_type: HuffmanTableType,
    pub destination_id: u8,
    pub bitcode_counts: [u8; 16],
    pub symbols: Vec<u8>,
    pub codes: Vec<u16>,
}

impl HuffmanTable {
    fn generate_codes(&mut self) {
        let mut code = 0;
        for code_count in self.bitcode_counts {
            for _ in 0..code_count {
                self.codes.push(code);
                code += 1;
            }
            code <<= 1;
        }
    }
}

#[derive(Debug)]
pub enum QuantizationTableType {
    Luma,
    Chroma,
}

#[derive(Debug)]
pub struct QuantizationTable {
    pub table_type: QuantizationTableType,
    pub precision: u8,
    pub destination_id: u8,
    pub table: [[u16; 8]; 8],
}

impl Default for QuantizationTable {
    fn default() -> Self {
        Self {
            table_type: QuantizationTableType::Luma,
            precision: 0,
            destination_id: 0,
            table: [[0; 8]; 8],
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct FrameComponent {
    pub identifier: u8,
    pub xy_sampling_factor: (u8, u8),
    pub qtable_id: u8,
}

#[derive(Debug, Default, Clone)]
pub struct ScanComponent {
    pub selector: u8,
    pub dc_table: u8,
    pub ac_table: u8,
}

#[derive(Debug, Default, Clone)]
pub struct Component {
    pub frame: FrameComponent,
    pub scan: ScanComponent,
}

#[derive(Debug, Default)]
pub struct ScanInfo {
    pub components: Vec<ScanComponent>,
    pub spectral_selection: (u8, u8),
    pub successive_approximation: u8,
}

#[derive(Debug, Default)]
pub struct FrameInfo {
    pub precision: u8,
    pub image_size: (u16, u16),
    pub padded_size: (u16, u16),
    pub components: Vec<FrameComponent>,
}

#[derive(Debug, Default)]
pub struct MCUInfo {
    pub max_xy_sampling_factor: (u8, u8),
    pub mcu_size: (u8, u8),
    pub mcu_dimensions: (u16, u16),
    pub mcu_padded_dimensions: (u16, u16),
}

#[derive(Debug, Default)]
pub struct HeaderInfo {
    pub frame_info: FrameInfo,
    pub scan_info: ScanInfo,
    pub components: Vec<Component>,
    pub ac_huff_tables: HashMap<u8, HuffmanTable>,
    pub dc_huff_tables: HashMap<u8, HuffmanTable>,
    pub quant_tables: HashMap<u8, QuantizationTable>,
    pub header_length: usize,
    pub mcu_info: MCUInfo,
}

impl HeaderInfo {
    fn read_start_of_frame(reader: &mut JPEGParser) -> Result<FrameInfo> {
        let _struct_size = reader.read_next_word()? - 2;

        let precision = reader.read_next_byte()?;

        let height = reader.read_next_word()?;
        let width = reader.read_next_word()?;

        let component_count = reader.read_next_byte()?;

        let mut components: Vec<FrameComponent> = Vec::with_capacity(component_count as usize);

        for _ in 0..component_count {
            let identifier = reader.read_next_byte()?;
            if components.iter().any(|c| c.identifier == identifier) {
                return Err(Error::Malformed("Duplicate component identifier"));
            }

            let sample_factors = reader.read_next_byte()?;
            let xy_sampling_factor = (sample_factors >> 4, sample_factors & 0x0F);

            let qtable_id = reader.read_next_byte()?;

            components.push(FrameComponent {
                identifier,
                xy_sampling_factor,
                qtable_id,
            })
        }

        Ok(FrameInfo {
            precision,
            image_size: (width, height),
            padded_size: (0, 0), // This can only be determined with info in the scan header
            components,
        })
    }

    fn read_quantization_tables(reader: &mut JPEGParser) -> Result<HashMap<u8, QuantizationTable>> {
        let struct_size = reader.read_next_word()? - 2;

        let mut quant_tables: HashMap<u8, QuantizationTable> = HashMap::new();

        let end_of_table = reader.position() + struct_size as u64;
        while reader.position() != end_of_table {
            let table_info = reader.read_next_byte()?;
            let precision = table_info >> 4;
            let destination_id = table_info & 0x0F;
            let table_type = match destination_id {
                0 => Ok(QuantizationTableType::Luma),
                1 => Ok(QuantizationTableType::Chroma),
                _ => Err(Error::UnsupportedFeature(
                    "Unsupported quantization table type",
                )),
            }?;

            let mut zagged_table = [0u16; 64];
            for value in zagged_table.iter_mut() {
                *value = match precision {
                    0 => reader.read_next_byte()? as u16,
                    1 => reader.read_next_word()?,
                    _ => return Err(Error::Malformed("Invalid precision value")),
                }
            }

            let mut unzagged_table = [[0u16; 8]; 8];
            for i in 0..zagged_table.len() {
                let (row, col) = ZIGZAG_MAP[i];
                unzagged_table[row as usize][col as usize] = zagged_table[i];
            }
            quant_tables.insert(
                destination_id,
                QuantizationTable {
                    table_type,
                    precision,
                    destination_id,
                    table: unzagged_table,
                },
            );
        }

        Ok(quant_tables)
    }

    fn read_huffman_tables(
        reader: &mut JPEGParser,
    ) -> Result<(HashMap<u8, HuffmanTable>, HashMap<u8, HuffmanTable>)> {
        let struct_size = reader.read_next_word()? - 2;

        let mut ac_tables: HashMap<u8, HuffmanTable> = HashMap::new();
        let mut dc_tables: HashMap<u8, HuffmanTable> = HashMap::new();

        let end_of_table = reader.position() + struct_size as u64;
        while reader.position() != end_of_table {
            let table_info = reader.read_next_byte()?;
            let table_type = match table_info >> 4 {
                0 => Ok(HuffmanTableType::Dc),
                1 => Ok(HuffmanTableType::Ac),
                _ => Err(Error::Malformed("Invalid table type")),
            }?;

            let destination_id = table_info & 0x0F;

            let mut bitcode_counts: [u8; 16] = [0; 16];

            for i in 0..16 {
                let count = reader.read_next_byte()?;
                bitcode_counts[i] = count;
            }

            let size: usize = bitcode_counts
                .iter()
                .fold(0, |total, elem| total + *elem as usize);

            let mut symbols = vec![0u8; size];
            for i in 0..size {
                symbols[i] = reader.read_next_byte()?;
            }

            let mut table = HuffmanTable {
                table_type,
                destination_id,
                bitcode_counts,
                symbols,
                codes: vec![],
            };

            table.generate_codes();

            match table.table_type {
                HuffmanTableType::Ac => ac_tables.insert(table.destination_id, table),
                HuffmanTableType::Dc => dc_tables.insert(table.destination_id, table),
            };
        }

        Ok((ac_tables, dc_tables))
    }

    /// Reads data from the scan header, leaving the cursor at the start of the scan stream.
    fn read_start_of_scan(reader: &mut JPEGParser) -> Result<ScanInfo> {
        let _struct_size = reader.read_next_word()? - 2;

        let component_count = reader.read_next_byte()?;

        let mut components = Vec::with_capacity(component_count as usize);
        for _ in 0..component_count {
            let selector = reader.read_next_byte()?;

            let tables = reader.read_next_byte()?;
            let dc_table = tables >> 4;
            let ac_table = tables & 0x0F;

            components.push(ScanComponent {
                selector,
                dc_table,
                ac_table,
            });
        }

        let spectral_selection_start = reader.read_next_byte()?;
        let spectral_selection_end = reader.read_next_byte()?;

        let successive_approximation = reader.read_next_byte()?;

        Ok(ScanInfo {
            components,
            spectral_selection: (spectral_selection_start, spectral_selection_end),
            successive_approximation,
        })
    }

    /// Reads header info from a given JPEGParser. The JPEGParser is expected to be at position 0
    /// in a JPEG data stream. It returns when it find the start of scan marker, reads its header,
    /// and leaves the cursor at the scan stream.
    pub fn read_header_info(reader: &mut JPEGParser) -> Result<Self> {
        {
            let marker = reader.read_next_marker()?;

            if marker != JPEGMarker::SOI {
                return Err(Error::Malformed(
                    "This JPEG image does not have an SOI marker",
                ));
            }
        }

        let mut result: Self = Default::default();

        loop {
            let marker = reader.read_next_marker()?;

            match marker {
                JPEGMarker::EOI => {
                    return Err(Error::Malformed("Unexpected EOI marker encountered."));
                }
                JPEGMarker::SOF0 => {
                    result.frame_info = Self::read_start_of_frame(reader)?;
                }
                JPEGMarker::DHT => {
                    let tables = Self::read_huffman_tables(reader)?;
                    result.ac_huff_tables.extend(tables.0);
                    result.dc_huff_tables.extend(tables.1);
                }
                JPEGMarker::DQT => {
                    result
                        .quant_tables
                        .extend(Self::read_quantization_tables(reader)?);
                }
                JPEGMarker::SOS => {
                    result.scan_info = Self::read_start_of_scan(reader)?;
                    result.header_length = reader.position() as usize;

                    {
                        result.mcu_info.max_xy_sampling_factor = result
                            .frame_info
                            .components
                            .iter()
                            .fold((0, 0), |(mut max_h_fac, mut max_v_fac), component| {
                                max_h_fac = max(component.xy_sampling_factor.0, max_h_fac);
                                max_v_fac = max(component.xy_sampling_factor.1, max_v_fac);

                                (max_h_fac, max_v_fac)
                            });
                    }

                    {
                        result.mcu_info.mcu_size = (
                            8 * result.mcu_info.max_xy_sampling_factor.0,
                            8 * result.mcu_info.max_xy_sampling_factor.1,
                        );

                        result.mcu_info.mcu_dimensions = (
                            result.frame_info.image_size.0 / result.mcu_info.mcu_size.0 as u16,
                            result.frame_info.image_size.1 / result.mcu_info.mcu_size.1 as u16,
                        );

                        result.frame_info.padded_size =
                            pad(result.frame_info.image_size, result.mcu_info.mcu_size);

                        result.mcu_info.mcu_padded_dimensions = (
                            result.frame_info.padded_size.0 / result.mcu_info.mcu_size.0 as u16,
                            result.frame_info.padded_size.1 / result.mcu_info.mcu_size.1 as u16,
                        );
                    }

                    {
                        if result.frame_info.components.len() != result.scan_info.components.len() {
                            return Err(Error::Malformed("Different number of components specified in scan header than frame header"));
                        }

                        result.components =
                            vec![Default::default(); result.frame_info.components.len()];

                        for i in 0..result.components.len() {
                            result.components[i].frame = result.frame_info.components[i].clone();
                            result.components[i].scan = result.scan_info.components[i].clone();
                        }
                    }

                    return Ok(result);
                }
                _ => {
                    reader.skip_marker_with_length()?; // Skip unkown markers
                }
            }
        }
    }
}

fn pad(unpadded: (u16, u16), block_size: (u8, u8)) -> (u16, u16) {
    let mut result = (0, 0);
    {
        let remainder = unpadded.0 % block_size.0 as u16;

        if remainder == 0 {
            result.0 = unpadded.0;
        } else {
            result.0 = unpadded.0 + block_size.0 as u16 - remainder;
        }
    }
    {
        let remainder = unpadded.1 % block_size.1 as u16;

        if remainder == 0 {
            result.1 = unpadded.1;
        } else {
            result.1 = unpadded.1 + block_size.1 as u16 - remainder;
        }
    }

    result
}
