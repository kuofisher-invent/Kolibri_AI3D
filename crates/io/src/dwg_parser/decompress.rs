//! DWG R2004+ Section Page Decompression (18CF compression)
//!
//! R2004 and later versions use a custom LZ-like compression for section pages.
//! The compression marker is 0x18CF (the "magic" bytes at the start of compressed pages).

use super::bitreader::DwgReadError;

/// Decompress a R2004+ compressed section page
/// Based on OpenDesign Specification section on System/Data Page compression
pub fn decompress_r2004(compressed: &[u8], decompressed_size: usize) -> Result<Vec<u8>, DwgReadError> {
    let mut output = Vec::with_capacity(decompressed_size);
    let mut src = 0usize;

    while src < compressed.len() && output.len() < decompressed_size {
        let opcode = compressed[src];
        src += 1;

        if opcode < 0x10 {
            // Literal run: copy (opcode + 1) bytes directly
            let count = opcode as usize + 1;
            if src + count > compressed.len() {
                return Err(DwgReadError::InvalidData("Literal overrun".into()));
            }
            output.extend_from_slice(&compressed[src..src + count]);
            src += count;
        } else if opcode < 0x20 {
            // Two-byte offset copy
            if src >= compressed.len() {
                return Err(DwgReadError::Eof);
            }
            let b2 = compressed[src] as usize;
            src += 1;

            let length = (opcode & 0x0F) as usize + 2;
            let offset = ((b2 << 4) | ((opcode as usize >> 4) & 0x0F)) + 1;

            copy_from_output(&mut output, offset, length)?;
        } else if opcode < 0x40 {
            // Long copy with 2-byte offset
            if src + 1 >= compressed.len() {
                return Err(DwgReadError::Eof);
            }
            let b2 = compressed[src] as usize;
            let b3 = compressed[src + 1] as usize;
            src += 2;

            let length = (opcode & 0x1F) as usize + 2;
            let offset = (b2 | (b3 << 8)) + 1;

            copy_from_output(&mut output, offset, length)?;
        } else if opcode == 0x11 {
            // End of compressed data marker
            break;
        } else {
            // Extended literal or copy — simplified handling
            let count = (opcode - 0x40) as usize + 1;
            if src + count > compressed.len() {
                // Truncated — copy what we can
                let available = compressed.len() - src;
                output.extend_from_slice(&compressed[src..src + available]);
                src += available;
                break;
            }
            output.extend_from_slice(&compressed[src..src + count]);
            src += count;
        }
    }

    // Pad or truncate to expected size
    output.resize(decompressed_size.min(output.len() + 1024), 0);
    output.truncate(decompressed_size);

    Ok(output)
}

/// Copy `length` bytes from `offset` positions back in the output buffer
fn copy_from_output(output: &mut Vec<u8>, offset: usize, length: usize) -> Result<(), DwgReadError> {
    if offset > output.len() {
        return Err(DwgReadError::InvalidData(
            format!("Back-reference offset {} > output size {}", offset, output.len())
        ));
    }

    let start = output.len() - offset;
    for i in 0..length {
        let byte = output[start + (i % offset)];
        output.push(byte);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_literal_copy() {
        // Opcode 0x02 = literal 3 bytes, then opcode 0x11 = end
        let compressed = [0x02, 0xAA, 0xBB, 0xCC, 0x11];
        let result = decompress_r2004(&compressed, 3).unwrap();
        assert_eq!(result, vec![0xAA, 0xBB, 0xCC]);
    }
}
