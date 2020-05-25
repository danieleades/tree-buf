pub mod delta;
pub mod packed_bool;
pub mod rle;
pub mod varint;
pub(crate) use rle::*;
mod compress;
use crate::prelude::*;
pub(crate) use compress::*;
pub mod gorilla;
/*
mod gorilla2;
pub mod gorilla {
    pub use super::gorilla2::*;
}
*/
//pub mod zfp;

#[cfg(feature = "write")]
pub(crate) struct Utf8Compressor;

#[cfg(feature = "read")]
/// Reads all items from some byte aligned encoding
pub fn read_all<T>(bytes: &[u8], f: impl Fn(&[u8], &mut usize) -> ReadResult<T>) -> ReadResult<Vec<T>> {
    profile!(T, "read_all");
    let mut offset = 0;
    let mut result = Vec::new();
    while offset < bytes.len() {
        let read = f(bytes, &mut offset)?;
        result.push(read);
    }
    debug_assert_eq!(offset, bytes.len());

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::Debug;
    #[cfg(all(feature = "read", feature = "write"))]
    pub fn round_trip<T: Copy + PartialEq + Debug>(data: &[T], encoder: impl Fn(T, &mut Vec<u8>), decoder: impl Fn(&[u8], &mut usize) -> ReadResult<T>) -> ReadResult<()> {
        let mut bytes = Vec::new();
        for value in data.iter() {
            encoder(*value, &mut bytes);
        }

        let result = read_all(&bytes, decoder)?;

        assert_eq!(&result, &data);
        Ok(())
    }
}
