//! Deflate meta payloads inflate to at most MAX_INFLATED_SIZE bytes.
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
fn data_neither_zlib_nor_raw_deflate_reports_the_zlib_error() {
    // A bad zlib header; a good zlib header over a reserved block type, which raw
    // deflate reads as a stored block whose LEN and NLEN disagree.
    for garbage in [
        vec![0xffu8, 0xff, 0xff, 0xff],
        vec![0x78u8, 0x01, 0x07, 0x07, 0x07, 0x00, 0x00, 0x00, 0x00],
    ] {
        let zlib_error = inflate::inflate_bytes_zlib(&garbage).unwrap_err();
        let raw_error = inflate::inflate_bytes(&garbage).unwrap_err();
        assert_ne!(zlib_error, raw_error, "{garbage:?}");
        match ContentEncoding::Deflate.decode(&garbage) {
            Err(Error::InflateError(message)) => assert_eq!(message, zlib_error, "{garbage:?}"),
            other => panic!("{garbage:?}: expected the zlib InflateError, got {other:?}"),
        }
    }
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
