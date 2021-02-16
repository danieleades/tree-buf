use crate::prelude::*;
use num_traits::AsPrimitive;

// TODO: Remove warning
#[allow(dead_code)]
pub fn decompress<T: 'static + Copy>(_bytes: &[u8]) -> DecodeResult<Vec<T>>
where
    f64: AsPrimitive<T>,
{
    todo!()
}

pub fn size_for(data: impl Iterator<Item = f64>) -> Result<usize, ()> {
    // FIXME: Verify current platform is little endian
    let mut data = data.map(f64::to_bits);
    // Initialized to 72 to account for the first value,
    // and 1 byte at end of "remaining bits"
    let mut bits = 72_usize;

    let buffer = match data.next() {
        Some(first) => first,
        None => return Err(()),
    };

    let mut previous = buffer;
    let mut prev_xor = buffer;

    // TODO: This was written this way to match output the existing gorilla compressor, and may not
    // match the actual paper. Investigate.
    for value in data {
        let xored = previous ^ value;

        if let 0 = xored {
            bits += 1;
        } else {
            let leading_zeros = xored.leading_zeros().min(31) as usize;
            let trailing_zeros = xored.trailing_zeros() as usize;
            let prev_leading_zeros = prev_xor.leading_zeros() as usize;
            let prev_trailing_zeros = if prev_leading_zeros == 64 { 0 } else { prev_xor.trailing_zeros() as usize };
            if leading_zeros >= prev_leading_zeros && trailing_zeros >= prev_trailing_zeros {
                bits += 66 - prev_trailing_zeros - prev_leading_zeros;
            } else {
                bits += 77 - trailing_zeros - leading_zeros;
            }
        }

        previous = value;
        prev_xor = xored;
    }

    let mut bytes = bits / 8;
    if bits % 8 != 0 {
        bytes += 1;
    }

    Ok(bytes)
}

pub fn compress(data: impl Iterator<Item = f64>, bytes: &mut Vec<u8>) -> Result<ArrayTypeId, ()> {
    // FIXME: Verify current platform is little endian
    let mut data = data.map(f64::to_bits);

    let encode = move |bits, count, capacity: &mut u8, buffer: &mut u64, bytes: &mut Vec<u8>| {
        if count <= *capacity {
            *buffer ^= bits << (*capacity - count);
            *capacity -= count;
        } else {
            let remainder = count - *capacity;
            // This check avoids a panic. Suprisingly >> doesn't truncate like
            // one might expect, and I didn't find an operator that did.
            if remainder != 64 {
                *buffer ^= bits >> remainder;
            }
            bytes.extend_from_slice(&buffer.to_le_bytes());
            *capacity = 64 - remainder;
            *buffer = bits << *capacity;
        }
    };

    let mut buffer = match data.next() {
        Some(first) => first,
        None => return Err(()),
    };

    let mut previous = buffer;
    let mut prev_xor = buffer;
    let mut capacity = 0;
    let capacity = &mut capacity;
    let buffer = &mut buffer;

    // TODO: This was written this way to match output the existing gorilla compressor, and may not
    // match the actual paper. Investigate.
    for value in data {
        let xored = previous ^ value;

        if let 0 = xored {
            encode(0, 1, capacity, buffer, bytes)
        } else {
            let leading_zeros = u64::from(xored.leading_zeros().min(31));
            let trailing_zeros = u64::from(xored.trailing_zeros());
            let prev_leading_zeros = u64::from(prev_xor.leading_zeros());
            let prev_trailing_zeros = if prev_leading_zeros == 64 { 0 } else { u64::from(prev_xor.trailing_zeros()) };
            if leading_zeros >= prev_leading_zeros && trailing_zeros >= prev_trailing_zeros {
                let meaningful_bits = xored >> prev_trailing_zeros;
                let meaningful_bit_count = 64 - prev_trailing_zeros - prev_leading_zeros;

                encode(0b10, 2, capacity, buffer, bytes);
                encode(meaningful_bits, meaningful_bit_count as u8, capacity, buffer, bytes);
            } else {
                let meaningful_bits = xored >> trailing_zeros;
                let meaningful_bit_count = 64 - trailing_zeros - leading_zeros;

                encode(0b11, 2, capacity, buffer, bytes);
                encode(leading_zeros, 5, capacity, buffer, bytes);
                encode(meaningful_bit_count - 1, 6, capacity, buffer, bytes);
                encode(meaningful_bits, meaningful_bit_count as u8, capacity, buffer, bytes);
            }
        }

        previous = value;
        prev_xor = xored;
    }

    // Add whatever is left
    let remaining = 64 - *capacity;
    let mut byte_count = remaining / 8;
    if byte_count * 8 != remaining {
        byte_count += 1;
    }
    let last = &(&buffer.to_le_bytes())[(8 - byte_count) as usize..];
    bytes.extend_from_slice(&last);
    bytes.push(remaining);

    Ok(ArrayTypeId::DoubleGorilla)
}
