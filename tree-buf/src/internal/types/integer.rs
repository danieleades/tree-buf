// TODO: Try Streaming V-Byte (which has a Rust port)
// https://lemire.me/blog/2017/09/27/stream-vbyte-breaking-new-speed-records-for-integer-compression/
use crate::internal::encodings::compress;
use crate::internal::encodings::varint::*;
use crate::prelude::*;
use num_traits::{AsPrimitive, Bounded};
use simple_16::Simple16;
use std::any::TypeId;
use std::convert::{TryFrom, TryInto};
use std::mem::transmute;
use std::vec::IntoIter;
use zigzag::ZigZag;

/// This serves to make the macro work in all cases having a "lower" type.
#[derive(Copy, Clone)]
pub struct U0;

impl Bounded for U0 {
    fn min_value() -> Self {
        U0
    }
    fn max_value() -> Self {
        U0
    }
}
mod _0 {
    use super::*;
    pub type Type = U0;

    pub fn encode_array<T, O: EncodeOptions>(_data: &[T], _max: T, _stream: &mut EncoderStream<'_, O>) -> ArrayTypeId {
        unreachable!();
    }
    pub fn fast_size_for_array<T, O>(_data: &[T], _max: T, _options: O) -> usize {
        unreachable!();
    }
}

macro_rules! impl_lowerable {
    ($Ty:ty, $mod_name:ident, $lower:ident, ($($lowers:ty),*), ($($compressions:ty),+)) => {
        mod $mod_name {
            use super::*;

            // This is allowed because nothing lowers to u64
            #[allow(dead_code)]
            pub type Type = $Ty;

            impl TryFrom<$Ty> for U0 {
                type Error=();
                fn try_from(_value: $Ty) -> Result<U0, Self::Error> {
                    Err(())
                }
            }
            impl TryFrom<U0> for $Ty {
                type Error=();
                fn try_from(_value: U0) -> Result<$Ty, Self::Error> {
                    Err(())
                }
            }
            impl AsPrimitive<U0> for $Ty {
                fn as_(self) -> U0 {
                    unreachable!()
                }
            }

            #[cfg(feature = "encode")]
            impl Encodable for $Ty {
                type EncoderArray = Vec<$Ty>;
                fn encode_root<O: EncodeOptions>(&self, stream: &mut EncoderStream<'_, O>) -> RootTypeId {
                    encode_root_uint(*self as u64, stream.bytes)
                }
            }



            #[cfg(feature = "encode")]
            impl EncoderArray<$Ty> for Vec<$Ty> {
                fn buffer_one<'a, 'b: 'a>(&'a mut self, value: &'b $Ty) {
                    self.push(*value);
                }
                fn buffer_many<'a, 'b: 'a>(&'a mut self, values: &'b [$Ty]) {
                    profile_method!(buffer_many);
                    self.extend_from_slice(values);
                }
                fn encode_all<O: EncodeOptions>(values: &[$Ty], stream: &mut EncoderStream<'_, O>) -> ArrayTypeId {
                    profile_method!(encode_all);
                    // TODO: (Performance) When getting ranges, use SIMD

                    let max = values.iter().max();
                    //dbg!(max);
                    if let Some(max) = max {
                        // TODO: (Performance) Use second-stack
                        // Lower to bool if possible. This is especially nice for enums
                        // with 2 variants.
                        if *max < 2 {
                            let bools = values.iter().map(|i| *i == 1).collect::<Vec<_>>();
                            bools.flush(stream)
                        } else {
                            encode_array(values, *max, stream)
                        }
                    } else {
                        ArrayTypeId::Void
                    }
                }
                fn flush<O: EncodeOptions>(self, stream: &mut EncoderStream<'_, O>) -> ArrayTypeId {
                    Self::encode_all(&self[..], stream)
                }
            }

        #[cfg(feature = "encode")]
        impl PrimitiveEncoderArray<$Ty> for Vec<$Ty> {
            fn fast_size_for_all<O: EncodeOptions>(values: &[$Ty], options: &O) -> usize {
                let max = values.iter().max();
                if let Some(max) = max {
                    // TODO: (Performance) Use second-stack
                        // Lower to bool if possible. This is especially nice for enums
                        // with 2 variants.
                        if *max < 2 {
                            let bools = values.iter().map(|i| *i == 1).collect::<Vec<_>>();
                            Vec::<bool>::fast_size_for_all(&bools[..], options)
                        } else {
                            fast_size_for_array(values, *max, options)
                        }
                    } else {
                        0
                    }
                }
            }

            #[cfg(feature = "decode")]
            impl Decodable for $Ty {
                type DecoderArray = IntoIter<$Ty>;
                fn decode(sticks: DynRootBranch<'_>, _options: &impl DecodeOptions) -> DecodeResult<Self> {
                    profile_method!(decode);
                    match sticks {
                        DynRootBranch::Integer(root_int) => {
                            match root_int {
                                RootInteger::U(v) => v.try_into().map_err(|_| DecodeError::SchemaMismatch),
                                _ => Err(DecodeError::SchemaMismatch),
                            }
                        }
                        _ => Err(DecodeError::SchemaMismatch),
                    }
                }
            }

            #[cfg(feature = "decode")]
            impl InfallibleDecoderArray for IntoIter<$Ty> {
                type Decode = $Ty;
                fn new_infallible(sticks: DynArrayBranch<'_>, options: &impl DecodeOptions) -> DecodeResult<Self> {
                    profile_method!(new_infallible);

                    match sticks {
                        DynArrayBranch::Integer(array_int) => {
                            let ArrayInteger { bytes, encoding } = array_int;
                            match encoding {
                                ArrayIntegerEncoding::PrefixVarInt => {
                                    profile_section!(prefix_var_int);

                                    let v: Vec<$Ty> = decode_all(
                                            &bytes,
                                            |bytes, offset| {
                                                let r: $Ty = decode_prefix_varint(bytes, offset)?.try_into().map_err(|_| DecodeError::SchemaMismatch)?;
                                                Ok(r)
                                            }
                                    )?;
                                    Ok(v.into_iter())
                                }
                                ArrayIntegerEncoding::Simple16 => {
                                    profile_section!(simple_16);

                                    let mut v = Vec::new();
                                    simple_16::decompress(&bytes, &mut v).map_err(|_| DecodeError::InvalidFormat)?;
                                    let result: Result<Vec<_>, _> = v.into_iter().map(TryInto::<$Ty>::try_into).collect();
                                    let v = result.map_err(|_| DecodeError::SchemaMismatch)?;
                                    Ok(v.into_iter())
                                },
                                ArrayIntegerEncoding::U8 => {
                                    profile_section!(fixed_u8);

                                    let v: Vec<$Ty> = bytes.iter().map(|&b| b.into()).collect();
                                    Ok(v.into_iter())
                                },
                                ArrayIntegerEncoding::DeltaZig => {
                                    profile_section!(delta_zig);
                                    let mut v = Vec::new();
                                    let mut prev: u32 = 0;
                                    let mut offset = 0;
                                    while offset < bytes.len() {
                                        // TODO: Not hardcoded to u32
                                        // See also e394b0c7-d5af-40b8-b944-cb68bac33fe9
                                        let next: u32 = decode_prefix_varint(&bytes, &mut offset)?.try_into().map_err(|_| DecodeError::InvalidFormat)?;
                                        let next: i32 = ZigZag::decode(next);
                                        let next = prev.wrapping_add(next as u32);
                                        prev = next;
                                        v.push(next.try_into().map_err(|_| DecodeError::InvalidFormat)?);
                                    }
                                    Ok(v.into_iter())
                                }
                            }
                        },
                        DynArrayBranch::RLE { runs, values } => {
                            let rle = RleIterator::new(runs, values, options, |values| Self::new_infallible(values, options))?;
                            let all = rle.collect::<Vec<_>>();
                            Ok(all.into_iter())
                        },
                        // FIXME: This fixes a particular test.
                        // It is unclear if this is canon.
                        // See also: 84d15459-35e4-4f04-896f-0f4ea9ce52a9
                        // TODO: Also apply this to other types
                        DynArrayBranch::Void => {
                            Ok(Vec::new().into_iter())
                        }
                        other => {
                            let bools = <IntoIter<bool> as InfallibleDecoderArray>::new_infallible(other, options)?;
                            let mapped = bools.map(|i| if i {1} else {0}).collect::<Vec<_>>();
                            Ok(mapped.into_iter())
                        },
                    }
                }
                fn decode_next_infallible(&mut self) -> Self::Decode {
                    self.next().unwrap_or_default()
                }
            }

            #[cfg(feature = "encode")]
            pub fn fast_size_for_array<O: EncodeOptions, T: Copy + std::fmt::Debug + AsPrimitive<$Ty> + AsPrimitive<U0> + AsPrimitive<u8> + AsPrimitive<$lower::Type> $(+ AsPrimitive<$lowers>),*>
                (data: &[T], max: T, options: &O) -> usize {

                let lower_max: Result<$Ty, _> = <$lower::Type as Bounded>::max_value().try_into();

                if let Ok(lower_max) = lower_max {
                    if lower_max >= max.as_() {
                        return $lower::fast_size_for_array(data, max, options)
                    }
                }

                fn fast_inner<O: EncodeOptions>(data: &[$Ty], options: &O, max: $Ty) -> usize {
                    let compressors = (
                        $(<$compressions>::new(max),)+
                        RLE::new(($(<$compressions>::new(max),)+))
                    );
                    fast_size_for(data, &compressors, options)
                }

                // Convert data to as<T>, using a transmute if that's already correct
                if TypeId::of::<$Ty>() == TypeId::of::<T>() {
                    // Safety - this is a unit conversion.
                    let data = unsafe { transmute(data) };
                    fast_inner(data, options, max.as_())
                } else {
                    // TODO: (Performance) Use second-stack
                    let v = {
                        profile_section!(copy_to_lowered);
                        data.iter().map(|i| i.as_()).collect::<Vec<_>>()
                    };
                    fast_inner(&v, options, max.as_())
                }
            }

            #[cfg(feature = "encode")]
            pub fn encode_array<O: EncodeOptions, T: Copy + std::fmt::Debug + AsPrimitive<$Ty> + AsPrimitive<U0> + AsPrimitive<u8> + AsPrimitive<$lower::Type> $(+ AsPrimitive<$lowers>),*>
                (data: &[T], max: T, stream: &mut EncoderStream<'_, O>) -> ArrayTypeId {

                let lower_max: Result<$Ty, _> = <$lower::Type as Bounded>::max_value().try_into();

                if let Ok(lower_max) = lower_max {
                    if lower_max >= max.as_() {
                        return $lower::encode_array(data, max, stream)
                    }
                }

                fn encode_inner<O: EncodeOptions>(data: &[$Ty], stream: &mut EncoderStream<'_, O>, max: $Ty) -> ArrayTypeId {
                    let compressors = (
                        $(<$compressions>::new(max),)+
                        RLE::new(($(<$compressions>::new(max),)+))
                    );
                    compress(data, stream, &compressors)
                }

                // Convert data to as<T>, using a transmute if that's already correct
                if TypeId::of::<$Ty>() == TypeId::of::<T>() {
                    // Safety - this is a unit conversion.
                    let data = unsafe { transmute(data) };
                    encode_inner(data, stream, max.as_())
                } else {
                    // TODO: (Performance) Use second-stack
                    let v = {
                        profile_section!(needless_lowered_copy);
                        data.iter().map(|i| i.as_()).collect::<Vec<_>>()
                    };
                    encode_inner(&v, stream, max.as_())
                }
            }
        }
    };
}

// TODO: This does all kinds of silly things. Eg: Perhaps we have u32 and simple16 is best.
// This may downcast to u16 then back up to u32. I'm afraid the final result is just going to
// be a bunch of hairy special code for each type with no generality.
//
// Broadly we only want to downcast if it allows for some other kind of compressor to be used.

// Type, array encoder, next lower, next lower encoder, non-inferred lowers
impl_lowerable!(u64, _64, _32, (u16), (PrefixVarIntCompressor));
impl_lowerable!(u32, _32, _16, (), (Simple16Compressor<u32>, DeltaZigZagCompressor, PrefixVarIntCompressor)); // TODO: Consider adding Fixed.
impl_lowerable!(u16, _16, _8, (), (Simple16Compressor<u16>, PrefixVarIntCompressor));
impl_lowerable!(u8, _8, _0, (), (Simple16Compressor<u8>, BytesCompressor));

#[cfg(feature = "encode")]
fn encode_root_uint(value: u64, bytes: &mut Vec<u8>) -> RootTypeId {
    let le = value.to_le_bytes();
    match value {
        0 => RootTypeId::Zero,
        1 => RootTypeId::One,
        2..=255 => {
            bytes.push(le[0]);
            RootTypeId::IntU8
        }
        256..=65535 => {
            bytes.extend_from_slice(&le[..2]);
            RootTypeId::IntU16
        }
        65536..=16777215 => {
            bytes.extend_from_slice(&le[..3]);
            RootTypeId::IntU24
        }
        16777216..=4294967295 => {
            bytes.extend_from_slice(&le[..4]);
            RootTypeId::IntU32
        }
        4294967296..=1099511627775 => {
            bytes.extend_from_slice(&le[..5]);
            RootTypeId::IntU40
        }
        1099511627776..=281474976710655 => {
            bytes.extend_from_slice(&le[..6]);
            RootTypeId::IntU48
        }
        281474976710656..=72057594037927936 => {
            bytes.extend_from_slice(&le[..7]);
            RootTypeId::IntU56
        }
        _ => {
            bytes.extend_from_slice(&le);
            RootTypeId::IntU64
        }
    }
}

// TODO: One-offing this isn't great.
// Get unsigned integers implemented
// TODO: Wrapping over smaller sizes
struct DeltaZigZagCompressor;
impl DeltaZigZagCompressor {
    #[inline(always)]
    pub fn new<T>(_max: T) -> Self {
        Self
    }
}

// TODO: Use second-stack
fn get_delta_zigs(data: &[u32]) -> Result<Vec<u32>, ()> {
    // TODO: Rename? This isn't really in rle
    within_rle(|| {
        if data.len() < 2 {
            return Err(());
        }
        let mut result = Vec::new();
        let mut current = 0;
        for next in data.iter() {
            // TODO: Not hard-coded to u32
            // See also e394b0c7-d5af-40b8-b944-cb68bac33fe9
            let diff = next.wrapping_sub(current) as i32;
            let zig = ZigZag::encode(diff);
            result.push(zig);
            current = *next;
        }
        Ok(result)
    })
}

impl Compressor<u32> for DeltaZigZagCompressor {
    fn compress<O: EncodeOptions>(&self, data: &[u32], stream: &mut EncoderStream<'_, O>) -> Result<ArrayTypeId, ()> {
        let deltas = get_delta_zigs(data)?;
        let _ignore_id = PrefixVarIntCompressor.compress(&deltas, stream);
        Ok(ArrayTypeId::DeltaZig)
    }
    fn fast_size_for<O: EncodeOptions>(&self, data: &[u32], options: &O) -> Result<usize, ()> {
        let deltas = get_delta_zigs(data)?;
        PrefixVarIntCompressor.fast_size_for(&deltas, options)
    }
}

struct PrefixVarIntCompressor;

impl PrefixVarIntCompressor {
    #[inline(always)]
    pub fn new<T>(_max: T) -> Self {
        Self
    }
}

impl<T: Into<u64> + Copy> Compressor<T> for PrefixVarIntCompressor {
    fn fast_size_for<O: EncodeOptions>(&self, data: &[T], _options: &O) -> Result<usize, ()> {
        profile_method!(fast_size_for);
        let mut size = 0;
        for item in data {
            size += size_for_varint((*item).into());
        }
        Ok(size)
    }
    fn compress<O: EncodeOptions>(&self, data: &[T], stream: &mut EncoderStream<'_, O>) -> Result<ArrayTypeId, ()> {
        profile_method!(compress);
        stream.encode_with_len(|stream| {
            for item in data {
                encode_prefix_varint((*item).into(), &mut stream.bytes);
            }
        });
        Ok(ArrayTypeId::IntPrefixVar)
    }
}

struct Simple16Compressor<T>(T);

impl<T> Simple16Compressor<T> {
    #[inline(always)]
    pub fn new(max: T) -> Self {
        Self(max)
    }
}

impl<T: Simple16> Simple16Compressor<T> {
    fn check_range(&self) -> Result<(), ()> {
        T::check(&[self.0]).map_err(|_| ())
    }
}

impl<T: Simple16 + PartialOrd> Compressor<T> for Simple16Compressor<T> {
    fn compress<O: EncodeOptions>(&self, data: &[T], stream: &mut EncoderStream<'_, O>) -> Result<ArrayTypeId, ()> {
        profile_method!(compress);

        self.check_range()?;

        stream.encode_with_len(|stream| unsafe { simple_16::compress_unchecked(&data, stream.bytes) });

        Ok(ArrayTypeId::IntSimple16)
    }

    fn fast_size_for<O: EncodeOptions>(&self, data: &[T], _options: &O) -> Result<usize, ()> {
        profile_method!(fast_size_for);

        self.check_range()?;

        let size = unsafe { simple_16::calculate_size_unchecked(&data) };

        Ok(size)
    }
}

struct BytesCompressor;
impl BytesCompressor {
    #[inline(always)]
    pub fn new<T>(_max: T) -> Self {
        Self
    }
}

impl Compressor<u8> for BytesCompressor {
    fn compress<O: EncodeOptions>(&self, data: &[u8], stream: &mut EncoderStream<'_, O>) -> Result<ArrayTypeId, ()> {
        profile_method!(compress);
        stream.encode_with_len(|stream| stream.bytes.extend_from_slice(data));
        Ok(ArrayTypeId::U8)
    }
    fn fast_size_for<O: EncodeOptions>(&self, data: &[u8], _options: &O) -> Result<usize, ()> {
        let len_size = size_for_varint(data.len() as u64);
        Ok(data.len() + len_size)
    }
}

// TODO: Bitpacking https://crates.io/crates/bitpacking
// TODO: Mayda https://crates.io/crates/mayda
// TODO: https://lemire.me/blog/2012/09/12/fast-integer-compression-decoding-billions-of-integers-per-second/
