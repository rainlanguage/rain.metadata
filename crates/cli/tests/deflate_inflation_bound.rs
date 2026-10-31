//! Deflate meta payloads inflate only as whole streams, to at most
//! MAX_INFLATED_SIZE bytes.
#![cfg(not(target_family = "wasm"))]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use rain_metadata::{
    ContentEncoding, ContentLanguage, ContentType, Error, KnownMagic, RainMetaDocumentV1Item,
    MAX_INFLATED_SIZE,
};

struct ThreadPeakAllocator;

thread_local! {
    static LIVE: Cell<usize> = const { Cell::new(0) };
    static PEAK: Cell<usize> = const { Cell::new(0) };
}

fn grow(size: usize) {
    let _ = LIVE.try_with(|live| {
        let now = live.get() + size;
        live.set(now);
        let _ = PEAK.try_with(|peak| peak.set(peak.get().max(now)));
    });
}

fn shrink(size: usize) {
    let _ = LIVE.try_with(|live| live.set(live.get().saturating_sub(size)));
}

unsafe impl GlobalAlloc for ThreadPeakAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = System.alloc(layout);
        if !ptr.is_null() {
            grow(layout.size());
        }
        ptr
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = System.alloc_zeroed(layout);
        if !ptr.is_null() {
            grow(layout.size());
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout);
        shrink(layout.size());
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_ptr = System.realloc(ptr, layout, new_size);
        if !new_ptr.is_null() {
            // A moving realloc holds both blocks at once.
            grow(new_size);
            shrink(layout.size());
        }
        new_ptr
    }
}

#[global_allocator]
static ALLOCATOR: ThreadPeakAllocator = ThreadPeakAllocator;

/// Runs `f` and returns its result with the peak heap growth on this thread
/// while it ran.
fn peak_heap_growth<T>(f: impl FnOnce() -> T) -> (T, usize) {
    let base = LIVE.with(Cell::get);
    PEAK.with(|peak| peak.set(base));
    let result = f();
    (result, PEAK.with(Cell::get) - base)
}

#[derive(Default)]
struct BitWriter {
    bytes: Vec<u8>,
    used: u8,
}

impl BitWriter {
    fn push_bit(&mut self, bit: bool) {
        if self.used == 0 {
            self.bytes.push(0);
        }
        if bit {
            *self.bytes.last_mut().unwrap() |= 1 << self.used;
        }
        self.used = (self.used + 1) % 8;
    }

    fn push_field(&mut self, value: u32, len: u8) {
        for i in 0..len {
            self.push_bit((value >> i) & 1 == 1);
        }
    }

    fn push_code(&mut self, (code, len): (u32, u8)) {
        for i in (0..len).rev() {
            self.push_bit((code >> i) & 1 == 1);
        }
    }
}

const FIXED_LITERAL_ZERO: (u32, u8) = (0b0011_0000, 8);
const FIXED_LENGTH_258: (u32, u8) = (0b1100_0101, 8);
const FIXED_DISTANCE_1: (u32, u8) = (0b00000, 5);
const FIXED_END_OF_BLOCK: (u32, u8) = (0b000_0000, 7);

/// One final fixed-Huffman raw deflate block (RFC 1951) inflating to `len`
/// zero bytes, built directly so a bomb never exists in inflated form.
fn raw_deflate_zeros(len: usize) -> Vec<u8> {
    let mut writer = BitWriter::default();
    writer.push_field(1, 1);
    writer.push_field(1, 2);
    if len > 0 {
        writer.push_code(FIXED_LITERAL_ZERO);
        for _ in 0..(len - 1) / 258 {
            writer.push_code(FIXED_LENGTH_258);
            writer.push_code(FIXED_DISTANCE_1);
        }
        for _ in 0..(len - 1) % 258 {
            writer.push_code(FIXED_LITERAL_ZERO);
        }
    }
    writer.push_code(FIXED_END_OF_BLOCK);
    writer.bytes
}

/// `raw_deflate_zeros` in a zlib wrapper (RFC 1950) with a 32 KiB window.
fn zlib_zeros(len: usize) -> Vec<u8> {
    let mut stream = vec![0x78, 0x01];
    stream.extend(raw_deflate_zeros(len));
    // Adler-32 of zero bytes: A stays 1, B counts the bytes.
    let adler = (((len % 65521) as u32) << 16) | 1;
    stream.extend(adler.to_be_bytes());
    stream
}

/// A raw deflate stored block (RFC 1951) whose first byte is `header`: bit 0 is
/// BFINAL, bits 1-2 are BTYPE 00, and the other five bits are padding.
fn stored_block(header: u8, data: &[u8]) -> Vec<u8> {
    let len = u16::try_from(data.len()).unwrap();
    let mut block = vec![header];
    block.extend(len.to_le_bytes());
    block.extend((!len).to_le_bytes());
    block.extend(data);
    block
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &byte in data {
        a = (a + u32::from(byte)) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

fn pseudo_random_bytes(len: usize, mut state: u64) -> Vec<u8> {
    (0..len)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state as u8
        })
        .collect()
}

fn assert_size_exceeded(result: Result<Vec<u8>, Error>) {
    match result {
        Err(Error::InflatedSizeExceeded(max)) => assert_eq!(max, MAX_INFLATED_SIZE),
        Err(other) => panic!("expected InflatedSizeExceeded, got {other:?}"),
        Ok(decoded) => panic!(
            "expected InflatedSizeExceeded, inflated {} bytes",
            decoded.len()
        ),
    }
}

fn assert_inflate_error(result: Result<Vec<u8>, Error>, input: &[u8]) {
    match result {
        Err(Error::InflateError(_)) => {}
        Err(other) => panic!("{input:02x?}: expected InflateError, got {other:?}"),
        Ok(decoded) => panic!(
            "{input:02x?}: expected InflateError, inflated {} bytes",
            decoded.len()
        ),
    }
}

#[test]
fn zlib_bomb_stops_at_the_bound_without_inflating_the_rest() {
    let bomb = zlib_zeros(16 * MAX_INFLATED_SIZE);
    assert!(bomb.len() < MAX_INFLATED_SIZE / 8);
    let (result, peak) = peak_heap_growth(|| ContentEncoding::Deflate.decode(&bomb));
    assert_size_exceeded(result);
    assert!(peak < 2 * MAX_INFLATED_SIZE, "peak heap growth {peak}");
}

#[test]
fn deflate_output_of_exactly_the_bound_inflates_and_one_more_byte_does_not() -> Result<(), Error>
{
    assert_eq!(MAX_INFLATED_SIZE, 16 * 1024 * 1024);
    let decoded = ContentEncoding::Deflate.decode(&zlib_zeros(MAX_INFLATED_SIZE))?;
    assert_eq!(decoded.len(), MAX_INFLATED_SIZE);
    assert!(decoded.iter().all(|&b| b == 0));
    drop(decoded);
    assert_size_exceeded(ContentEncoding::Deflate.decode(&zlib_zeros(MAX_INFLATED_SIZE + 1)));
    Ok(())
}

#[test]
fn raw_deflate_that_is_not_zlib_inflates_through_the_fallback() -> Result<(), Error> {
    let zeros = raw_deflate_zeros(100_000);
    assert!(inflate::inflate_bytes_zlib(&zeros).is_err());
    assert_eq!(ContentEncoding::Deflate.decode(&zeros)?, vec![0u8; 100_000]);

    let mut content = pseudo_random_bytes(70_000, 0x9e37_79b9_7f4a_7c15);
    content.extend(b"rain meta ".repeat(10_000));
    let raw = deflate::deflate_bytes(&content);
    assert!(inflate::inflate_bytes_zlib(&raw).is_err());
    assert_eq!(ContentEncoding::Deflate.decode(&raw)?, content);
    Ok(())
}

#[test]
fn raw_deflate_bomb_stops_at_the_bound_without_inflating_the_rest() {
    let bomb = raw_deflate_zeros(16 * MAX_INFLATED_SIZE);
    assert!(inflate::InflateStream::from_zlib().update(&bomb).is_err());
    let (result, peak) = peak_heap_growth(|| ContentEncoding::Deflate.decode(&bomb));
    assert_size_exceeded(result);
    assert!(peak < 2 * MAX_INFLATED_SIZE, "peak heap growth {peak}");
}

#[test]
fn data_neither_zlib_nor_raw_deflate_is_an_inflate_error() {
    for garbage in [
        vec![],
        vec![0x78u8],
        vec![0xffu8, 0xff, 0xff, 0xff],
        // A zlib header with no deflate data, then one with a cut stored block.
        vec![0x78u8, 0x01],
        vec![0x78u8, 0x9c, 0x00],
        // A good zlib header over a reserved block type, which raw deflate reads
        // as a stored block whose LEN and NLEN disagree.
        vec![0x78u8, 0x01, 0x07, 0x07, 0x07],
        vec![0x78u8, 0x01, 0x07, 0x07, 0x07, 0x00, 0x00, 0x00, 0x00],
        // A zlib header asking for a preset dictionary.
        vec![0x78u8, 0x20, 0x00, 0x00, 0x00, 0x00],
    ] {
        assert_inflate_error(ContentEncoding::Deflate.decode(&garbage), &garbage);
    }
}

#[test]
fn zlib_truncated_before_the_end_of_its_deflate_data_is_an_error() -> Result<(), Error> {
    let mut content = b"#calculate-io\n_ _: 1 2;\n".repeat(40);
    content.extend(pseudo_random_bytes(500, 4));
    let encoded = ContentEncoding::Deflate.encode(&content);
    assert_eq!(ContentEncoding::Deflate.decode(&encoded)?, content);
    for cut in 0..encoded.len() - 4 {
        assert_inflate_error(
            ContentEncoding::Deflate.decode(&encoded[..cut]),
            &encoded[..cut],
        );
    }

    // Cut exactly after a complete non-final block.
    let first = b"rain meta first block";
    let mut content = first.to_vec();
    content.extend(vec![0u8; 1000]);
    let mut stream = vec![0x78, 0x01];
    stream.extend(stored_block(0x00, first));
    let boundary = stream.len();
    stream.extend(raw_deflate_zeros(1000));
    stream.extend(adler32(&content).to_be_bytes());
    assert_eq!(ContentEncoding::Deflate.decode(&stream)?, content);
    assert_inflate_error(
        ContentEncoding::Deflate.decode(&stream[..boundary]),
        &stream[..boundary],
    );
    Ok(())
}

#[test]
fn zlib_missing_all_or_part_of_its_adler32_trailer_is_an_error() {
    let mut content = pseudo_random_bytes(5_000, 5);
    content.extend(b"rain meta ".repeat(500));
    for (encoded, content) in [
        (ContentEncoding::Deflate.encode(&content), content),
        (zlib_zeros(1000), vec![0u8; 1000]),
    ] {
        let deflate_end = encoded.len() - 4;
        assert_eq!(
            inflate::inflate_bytes(&encoded[2..deflate_end]).unwrap(),
            content
        );
        for dropped in 1..=4 {
            let cut = &encoded[..encoded.len() - dropped];
            assert_inflate_error(ContentEncoding::Deflate.decode(cut), cut);
        }
    }
}

#[test]
fn zlib_with_a_wrong_adler32_is_an_error() -> Result<(), Error> {
    let mut content = pseudo_random_bytes(5_000, 6);
    content.extend(b"rain meta ".repeat(500));
    for encoded in [ContentEncoding::Deflate.encode(&content), zlib_zeros(1000)] {
        ContentEncoding::Deflate.decode(&encoded)?;
        for byte in encoded.len() - 4..encoded.len() {
            for bit in 0..8 {
                let mut corrupt = encoded.clone();
                corrupt[byte] ^= 1 << bit;
                assert_inflate_error(ContentEncoding::Deflate.decode(&corrupt), &corrupt);
            }
        }
    }
    Ok(())
}

#[test]
fn raw_deflate_whose_first_two_bytes_pass_the_zlib_header_check_inflates() {
    // A non-final stored block's first byte has five padding bits, so it can hold
    // CM 8 and CINFO <= 7, and with LEN's low byte as FLG the first two bytes are
    // a valid zlib CMF/FLG pair.
    let mut rejected = Vec::new();
    let mut cases = 0;
    for cinfo in 0..=7u8 {
        let cmf = (cinfo << 4) | 8;
        for flg in (0..=255u8).filter(|&flg| u16::from_be_bytes([cmf, flg]) % 31 == 0) {
            cases += 1;
            let first: Vec<u8> = (0..flg).map(|i| b'a' + i % 26).collect();
            let mut content = first.clone();
            content.extend(vec![0u8; 1000]);
            let mut raw = stored_block(cmf, &first);
            raw.extend(raw_deflate_zeros(1000));
            assert_eq!(raw[..2], [cmf, flg]);
            assert!(
                inflate::inflate_bytes_zlib(&raw).is_err(),
                "{:02x?}",
                &raw[..5]
            );
            assert_eq!(inflate::inflate_bytes(&raw).unwrap(), content);
            match ContentEncoding::Deflate.decode(&raw) {
                Ok(decoded) if decoded == content => {}
                other => rejected.push(format!(
                    "{:02x?}: {:?}",
                    &raw[..5],
                    other.map(|decoded| decoded.len())
                )),
            }
        }
    }
    assert!(
        rejected.is_empty(),
        "{} of {cases} rejected: {rejected:?}",
        rejected.len()
    );
}

#[test]
fn deflate_encode_then_decode_round_trips() -> Result<(), Error> {
    let window = 32 * 1024;
    let contents = [
        vec![],
        vec![0x00],
        b"hello rain deflate".to_vec(),
        pseudo_random_bytes(window, 1),
        pseudo_random_bytes(window + 1, 2),
        pseudo_random_bytes(300_000, 3),
        b"#calculate-io\n_ _: 1 2;\n".repeat(80_000),
    ];
    for content in contents {
        let encoded = ContentEncoding::Deflate.encode(&content);
        assert_eq!(
            ContentEncoding::Deflate.decode(&encoded)?,
            content,
            "{} bytes",
            content.len()
        );
    }
    Ok(())
}

#[test]
fn unpack_of_an_item_over_the_bound_surfaces_the_size_error() {
    let item = RainMetaDocumentV1Item {
        payload: serde_bytes::ByteBuf::from(zlib_zeros(MAX_INFLATED_SIZE + 1)),
        magic: KnownMagic::DotrainV1,
        content_type: ContentType::OctetStream,
        content_encoding: ContentEncoding::Deflate,
        content_language: ContentLanguage::None,
        schema: None,
    };
    assert_size_exceeded(item.unpack());
}
