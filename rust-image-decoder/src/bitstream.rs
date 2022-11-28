use crate::error::Error;

/// Bitstream reader. Reads arbitrary bits out of a bitstream without respect to endianness.
#[derive(Debug)]
pub struct Bitstream<'data> {
    data: &'data [u8],
    byte_cursor: usize,
    bit_cursor: u8,
}

impl<'data> Bitstream<'data> {
    /// Creates a new bitstream.
    pub fn new(data: &'data [u8]) -> Self {
        Self {
            data,
            byte_cursor: 0,
            bit_cursor: 0,
        }
    }

    // TODO: Figure out if this is actually needed
    /* Currently unused
    /// Returns the current cursor position in the bitstream in terms of its "bit index"
    pub fn get_cursor_position(&self) -> usize {
        self.byte_cursor * 8 + (self.bit_cursor as usize)
    }
    */

    // TODO: Figure out if this is actually needed
    /* Currently unused
    /// Sets the cursor's position in the bitstream
    pub fn set_cursor(&mut self, bit_position: usize) -> Result<(), Error> {
        self.byte_cursor = bit_position / 8;
        self.bit_cursor = (bit_position % 8) as u8;

        if self.byte_cursor > self.data.len()
            || (self.byte_cursor == self.data.len() && self.bit_cursor > 0)
        {
            return Err(Error::Malformed("Bit cursor advanced past end of data"));
        }
        Ok(())
    }
    */

    // TODO: Figure out if this is actually needed
    /* Currently unused
    /// Advances the cursor by a given number of bits.
    pub fn advance_cursor(&mut self, bit_step: usize) -> Result<(), Error> {
        self.set_cursor(self.get_cursor_position() + bit_step)
    }
    */

    /// Reads up to 64 bits out of the bitstream and returns them in a u64.
    pub fn read_bits(&mut self, bits: usize) -> Result<u64, Error> {
        if bits > 64 {
            return Err(Error::InternalError(
                "Can't read more than 64 bits at a time",
            ));
        }

        if self.byte_cursor >= self.data.len() {
            return Err(Error::InternalError("Read past end of bit buffer"));
        }

        let mut value: u64 = 0;
        for _ in 0..bits {
            let current_byte = self.data[self.byte_cursor];
            let current_bit = 1u8 & (current_byte >> (7 - self.bit_cursor));

            value = (value << 1) | current_bit as u64;

            self.bit_cursor += 1;
            if self.bit_cursor == 8 {
                self.byte_cursor += 1;
                self.bit_cursor = 0;

                if self.byte_cursor > self.data.len() {
                    return Err(Error::InternalError("Read past end of bit buffer"));
                }
            }
        }
        Ok(value)
    }
}
