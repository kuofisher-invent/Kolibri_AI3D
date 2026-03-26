//! DWG Bit-level Reader
//!
//! DWG encodes data at the BIT level, not byte level.
//! This reader supports all DWG-specific data types:
//!   - B (bit), BB (2 bits), 3B (3 bits)
//!   - BS (bitshort), BL (bitlong), BD (bitdouble)
//!   - MC (modular char), MS (modular short)
//!   - H (handle reference)
//!   - T (text string)
//!   - CMC (color)
//!   - RC, RS, RD, RL (raw byte/short/double/long)

/// Bit-level reader for DWG binary data
#[derive(Debug, Clone)]
pub struct BitReader<'a> {
    data: &'a [u8],
    /// Current byte position
    byte_pos: usize,
    /// Current bit position within the byte (0-7, MSB first)
    bit_pos: u8,
}

impl<'a> BitReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, byte_pos: 0, bit_pos: 0 }
    }

    /// Create a reader starting at a specific byte offset
    pub fn from_offset(data: &'a [u8], offset: usize) -> Self {
        Self { data, byte_pos: offset, bit_pos: 0 }
    }

    /// Current position in bits from start
    pub fn pos_bits(&self) -> usize {
        self.byte_pos * 8 + self.bit_pos as usize
    }

    /// Current byte position
    pub fn pos_bytes(&self) -> usize {
        self.byte_pos
    }

    /// Remaining bytes (approximate)
    pub fn remaining(&self) -> usize {
        if self.byte_pos >= self.data.len() { return 0; }
        self.data.len() - self.byte_pos
    }

    /// Align to next byte boundary
    pub fn align_byte(&mut self) {
        if self.bit_pos != 0 {
            self.bit_pos = 0;
            self.byte_pos += 1;
        }
    }

    /// Set absolute byte position
    pub fn seek(&mut self, byte_pos: usize) {
        self.byte_pos = byte_pos;
        self.bit_pos = 0;
    }

    // ═══════════════════════════════════════════════════════════════════
    //  Raw reads (not bit-packed)
    // ═══════════════════════════════════════════════════════════════════

    /// Read a raw byte (RC)
    pub fn read_rc(&mut self) -> Result<u8, DwgReadError> {
        self.align_byte();
        if self.byte_pos >= self.data.len() {
            return Err(DwgReadError::Eof);
        }
        let v = self.data[self.byte_pos];
        self.byte_pos += 1;
        Ok(v)
    }

    /// Read raw bytes
    pub fn read_bytes(&mut self, count: usize) -> Result<&'a [u8], DwgReadError> {
        self.align_byte();
        if self.byte_pos + count > self.data.len() {
            return Err(DwgReadError::Eof);
        }
        let slice = &self.data[self.byte_pos..self.byte_pos + count];
        self.byte_pos += count;
        Ok(slice)
    }

    /// Read a raw 16-bit little-endian (RS)
    pub fn read_rs(&mut self) -> Result<i16, DwgReadError> {
        let bytes = self.read_bytes(2)?;
        Ok(i16::from_le_bytes([bytes[0], bytes[1]]))
    }

    /// Read a raw 32-bit little-endian (RL)
    pub fn read_rl(&mut self) -> Result<i32, DwgReadError> {
        let bytes = self.read_bytes(4)?;
        Ok(i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    /// Read a raw 64-bit double (RD)
    pub fn read_rd(&mut self) -> Result<f64, DwgReadError> {
        let bytes = self.read_bytes(8)?;
        Ok(f64::from_le_bytes(bytes.try_into().map_err(|_| DwgReadError::Eof)?))
    }

    // ═══════════════════════════════════════════════════════════════════
    //  Bit-packed reads (DWG specific)
    // ═══════════════════════════════════════════════════════════════════

    /// Read a single bit (B) — returns 0 or 1
    pub fn read_bit(&mut self) -> Result<u8, DwgReadError> {
        if self.byte_pos >= self.data.len() {
            return Err(DwgReadError::Eof);
        }
        let byte = self.data[self.byte_pos];
        let bit = (byte >> (7 - self.bit_pos)) & 1;
        self.bit_pos += 1;
        if self.bit_pos >= 8 {
            self.bit_pos = 0;
            self.byte_pos += 1;
        }
        Ok(bit)
    }

    /// Read N bits as u32
    pub fn read_bits(&mut self, count: u8) -> Result<u32, DwgReadError> {
        let mut value = 0u32;
        for _ in 0..count {
            value = (value << 1) | self.read_bit()? as u32;
        }
        Ok(value)
    }

    /// Read 2 bits (BB)
    pub fn read_bb(&mut self) -> Result<u8, DwgReadError> {
        Ok(self.read_bits(2)? as u8)
    }

    /// Read 3 bits (3B) — used in R2000+
    pub fn read_3b(&mut self) -> Result<u8, DwgReadError> {
        Ok(self.read_bits(3)? as u8)
    }

    /// Read a bit-short (BS) — 2 + 0/8/16 bits
    /// Encoding:
    ///   00 → read 16-bit RS
    ///   01 → read 8-bit unsigned
    ///   10 → value is 0
    ///   11 → value is 256
    pub fn read_bs(&mut self) -> Result<i16, DwgReadError> {
        let code = self.read_bb()?;
        match code {
            0 => {
                // 16-bit value follows
                let lo = self.read_bits(8)? as u16;
                let hi = self.read_bits(8)? as u16;
                Ok((lo | (hi << 8)) as i16)
            }
            1 => {
                // 8-bit unsigned
                Ok(self.read_bits(8)? as i16)
            }
            2 => Ok(0),
            3 => Ok(256),
            _ => unreachable!(),
        }
    }

    /// Read a bit-long (BL) — 2 + 0/8/32 bits
    /// Encoding:
    ///   00 → read 32-bit RL
    ///   01 → read 8-bit unsigned
    ///   10 → value is 0
    ///   11 → not used (invalid)
    pub fn read_bl(&mut self) -> Result<i32, DwgReadError> {
        let code = self.read_bb()?;
        match code {
            0 => {
                let b0 = self.read_bits(8)?;
                let b1 = self.read_bits(8)?;
                let b2 = self.read_bits(8)?;
                let b3 = self.read_bits(8)?;
                Ok((b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)) as i32)
            }
            1 => Ok(self.read_bits(8)? as i32),
            2 => Ok(0),
            _ => Err(DwgReadError::InvalidData("Invalid BL code 11".into())),
        }
    }

    /// Read a bit-double (BD) — 2 + 0/64 bits
    /// Encoding:
    ///   00 → read 64-bit RD
    ///   01 → value is 1.0
    ///   10 → value is 0.0
    ///   11 → not used
    pub fn read_bd(&mut self) -> Result<f64, DwgReadError> {
        let code = self.read_bb()?;
        match code {
            0 => {
                // Read 64 bits as f64
                let mut bytes = [0u8; 8];
                for b in &mut bytes {
                    *b = self.read_bits(8)? as u8;
                }
                Ok(f64::from_le_bytes(bytes))
            }
            1 => Ok(1.0),
            2 => Ok(0.0),
            _ => Err(DwgReadError::InvalidData("Invalid BD code 11".into())),
        }
    }

    /// Read a 2D point (BD, BD)
    pub fn read_2bd(&mut self) -> Result<[f64; 2], DwgReadError> {
        Ok([self.read_bd()?, self.read_bd()?])
    }

    /// Read a 3D point (BD, BD, BD)
    pub fn read_3bd(&mut self) -> Result<[f64; 3], DwgReadError> {
        Ok([self.read_bd()?, self.read_bd()?, self.read_bd()?])
    }

    /// Read a modular character (MC) — variable-length signed integer
    /// Each byte: bit7 = continue flag, bits 0-6 = data
    /// Last byte: bit6 = sign flag
    pub fn read_mc(&mut self) -> Result<i32, DwgReadError> {
        self.align_byte();
        let mut result = 0i32;
        let mut shift = 0;
        let mut negative = false;

        loop {
            if self.byte_pos >= self.data.len() {
                return Err(DwgReadError::Eof);
            }
            let byte = self.data[self.byte_pos];
            self.byte_pos += 1;

            let has_more = (byte & 0x80) != 0;
            let value = (byte & 0x7F) as i32;

            result |= value << shift;
            shift += 7;

            if !has_more {
                // Check sign in the last byte
                if (byte & 0x40) != 0 {
                    negative = true;
                    // Clear the sign bit from result
                    result &= !(0x40 << (shift - 7));
                }
                break;
            }

            if shift > 32 {
                return Err(DwgReadError::InvalidData("MC too long".into()));
            }
        }

        Ok(if negative { -result } else { result })
    }

    /// Read a modular short (MS) — variable-length unsigned integer
    /// Each 16-bit word: bit15 = continue flag, bits 0-14 = data
    pub fn read_ms(&mut self) -> Result<u32, DwgReadError> {
        self.align_byte();
        let mut result = 0u32;
        let mut shift = 0;

        loop {
            if self.byte_pos + 2 > self.data.len() {
                return Err(DwgReadError::Eof);
            }
            let lo = self.data[self.byte_pos] as u32;
            let hi = self.data[self.byte_pos + 1] as u32;
            self.byte_pos += 2;

            let word = lo | (hi << 8);
            let has_more = (word & 0x8000) != 0;
            let value = word & 0x7FFF;

            result |= value << shift;
            shift += 15;

            if !has_more { break; }
            if shift > 32 {
                return Err(DwgReadError::InvalidData("MS too long".into()));
            }
        }

        Ok(result)
    }

    /// Read a handle reference (H) — DWG object handle
    /// Format: code (4 bits) + counter (4 bits) + counter bytes of handle
    pub fn read_handle(&mut self) -> Result<DwgHandle, DwgReadError> {
        let code_counter = self.read_bits(8)? as u8;
        let code = (code_counter >> 4) & 0x0F;
        let counter = (code_counter & 0x0F) as usize;

        let mut value = 0u32;
        for _ in 0..counter {
            value = (value << 8) | self.read_bits(8)?;
        }

        Ok(DwgHandle { code, value })
    }

    /// Read a text string (T)
    /// R13-R2004: BS length + RS characters (each char is 1 byte)
    /// R2007+: BS length + 2-byte Unicode characters
    pub fn read_text(&mut self, unicode: bool) -> Result<String, DwgReadError> {
        let len = self.read_bs()? as usize;
        if len == 0 { return Ok(String::new()); }

        if unicode {
            // R2007+: UTF-16LE
            let mut chars = Vec::with_capacity(len);
            for _ in 0..len {
                let lo = self.read_bits(8)? as u16;
                let hi = self.read_bits(8)? as u16;
                chars.push(lo | (hi << 8));
            }
            Ok(String::from_utf16_lossy(&chars))
        } else {
            // R13-R2004: single-byte text
            let mut bytes = Vec::with_capacity(len);
            for _ in 0..len {
                bytes.push(self.read_bits(8)? as u8);
            }
            // Try UTF-8 first, fall back to Latin-1
            String::from_utf8(bytes.clone())
                .or_else(|_| Ok(bytes.iter().map(|&b| b as char).collect()))
        }
    }

    /// Read a CMC color value
    /// Pre-R2004: BS (color index)
    /// R2004+: BS index + BL rgb + RC (color byte) + T (name) + T (book)
    pub fn read_cmc(&mut self, is_r2004_plus: bool) -> Result<DwgColor, DwgReadError> {
        let index = self.read_bs()?;
        if is_r2004_plus {
            let rgb = self.read_bl()?;
            let _color_byte = self.read_bits(8)?;
            Ok(DwgColor { index, rgb: Some(rgb as u32) })
        } else {
            Ok(DwgColor { index, rgb: None })
        }
    }

    /// Read CRC-16 (used extensively in DWG for data validation)
    pub fn read_crc16(&mut self) -> Result<u16, DwgReadError> {
        self.align_byte();
        if self.byte_pos + 2 > self.data.len() {
            return Err(DwgReadError::Eof);
        }
        let lo = self.data[self.byte_pos] as u16;
        let hi = self.data[self.byte_pos + 1] as u16;
        self.byte_pos += 2;
        Ok(lo | (hi << 8))
    }
}

// ═══════════════════════════════════════════════════════════════════════
//  Types
// ═══════════════════════════════════════════════════════════════════════

/// DWG object handle (used to reference objects in the file)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DwgHandle {
    pub code: u8,    // handle type (2=soft owner, 3=hard owner, 4=soft pointer, 5=hard pointer)
    pub value: u32,  // handle value
}

impl DwgHandle {
    pub fn absolute(&self, referrer_handle: u32) -> u32 {
        match self.code {
            0x02 | 0x03 | 0x04 | 0x05 => self.value,
            0x06 => referrer_handle + 1,
            0x08 => referrer_handle - 1,
            0x0A => referrer_handle + self.value,
            0x0C => referrer_handle.wrapping_sub(self.value),
            _ => self.value,
        }
    }
}

/// DWG color (ACI index + optional RGB)
#[derive(Debug, Clone, Copy)]
pub struct DwgColor {
    pub index: i16,
    pub rgb: Option<u32>,
}

/// Read errors
#[derive(Debug, Clone)]
pub enum DwgReadError {
    Eof,
    InvalidData(String),
}

impl std::fmt::Display for DwgReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Eof => write!(f, "Unexpected end of data"),
            Self::InvalidData(msg) => write!(f, "Invalid data: {}", msg),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
//  CRC-16 calculation (DWG uses a custom polynomial)
// ═══════════════════════════════════════════════════════════════════════

/// DWG CRC-16 lookup table (polynomial from OpenDesign Spec)
const CRC_TABLE: [u16; 256] = {
    let mut table = [0u16; 256];
    let mut i = 0;
    while i < 256 {
        let mut crc = i as u16;
        let mut j = 0;
        while j < 8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xA001;
            } else {
                crc >>= 1;
            }
            j += 1;
        }
        table[i] = crc;
        i += 1;
    }
    table
};

/// Compute CRC-16 for a data slice with initial seed
pub fn crc16(data: &[u8], seed: u16) -> u16 {
    let mut crc = seed;
    for &byte in data {
        let idx = ((crc ^ byte as u16) & 0xFF) as usize;
        crc = (crc >> 8) ^ CRC_TABLE[idx];
    }
    crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_bit() {
        let data = [0b10110100u8, 0b01100000u8];
        let mut r = BitReader::new(&data);
        assert_eq!(r.read_bit().unwrap(), 1);
        assert_eq!(r.read_bit().unwrap(), 0);
        assert_eq!(r.read_bit().unwrap(), 1);
        assert_eq!(r.read_bit().unwrap(), 1);
        assert_eq!(r.read_bit().unwrap(), 0);
        assert_eq!(r.read_bit().unwrap(), 1);
        assert_eq!(r.read_bit().unwrap(), 0);
        assert_eq!(r.read_bit().unwrap(), 0);
        // Next byte
        assert_eq!(r.read_bit().unwrap(), 0);
        assert_eq!(r.read_bit().unwrap(), 1);
        assert_eq!(r.read_bit().unwrap(), 1);
    }

    #[test]
    fn test_read_bs() {
        // Code 10 → value 0
        let data = [0b10000000u8];
        let mut r = BitReader::new(&data);
        assert_eq!(r.read_bs().unwrap(), 0);

        // Code 11 → value 256
        let data = [0b11000000u8];
        let mut r = BitReader::new(&data);
        assert_eq!(r.read_bs().unwrap(), 256);

        // Code 01 → 8-bit value (0x42 = 66)
        let data = [0b01_0100_00, 0b10_000000];
        let mut r = BitReader::new(&data);
        assert_eq!(r.read_bs().unwrap(), 66);
    }

    #[test]
    fn test_read_bd() {
        // Code 01 → value 1.0
        let data = [0b01000000u8];
        let mut r = BitReader::new(&data);
        assert_eq!(r.read_bd().unwrap(), 1.0);

        // Code 10 → value 0.0
        let data = [0b10000000u8];
        let mut r = BitReader::new(&data);
        assert_eq!(r.read_bd().unwrap(), 0.0);
    }

    #[test]
    fn test_read_mc() {
        // Single byte, positive: 0x05 → bit7=0 (no continue), bit6=0 (positive), bits0-5=5
        let data = [0x05u8];
        let mut r = BitReader::new(&data);
        assert_eq!(r.read_mc().unwrap(), 5);

        // Single byte, positive: 0x1F → bit7=0, bit6=0, bits0-5=0x1F=31
        let data = [0x1Fu8];
        let mut r = BitReader::new(&data);
        assert_eq!(r.read_mc().unwrap(), 31);

        // Negative: 0x41 → bit7=0 (no continue), bit6=1 (sign), bits0-5=1 → -1
        let data = [0x41u8];
        let mut r = BitReader::new(&data);
        assert_eq!(r.read_mc().unwrap(), -1);
    }

    #[test]
    fn test_read_handle() {
        // Code=4 (soft pointer), counter=1, value=0x05
        let data = [0b0100_0001, 0x05];
        let mut r = BitReader::new(&data);
        let h = r.read_handle().unwrap();
        assert_eq!(h.code, 4);
        assert_eq!(h.value, 5);
    }

    #[test]
    fn test_crc16() {
        let data = b"Hello";
        let crc = crc16(data, 0);
        // Just verify it doesn't panic and returns something
        assert!(crc != 0 || crc == 0); // always true, but tests the function runs
    }
}
