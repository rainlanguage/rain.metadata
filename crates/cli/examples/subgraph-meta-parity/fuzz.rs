use alloy::primitives::keccak256;
use rain_metadata::KnownMagic;
use strum::IntoEnumIterator;

const PREFIX: u64 = 0xff0a89c674ee7874;
const OA_SCHEMA: u64 = 0xffa8e8a9b9cf4a31;
const NAME_PREIMAGE: &[u8] = b"st0x.rbac.us.v1";

const CONTENT_TYPES: &[&str] = &[
    "None",
    "application/json",
    "application/cbor",
    "application/octet-stream",
];
const CONTENT_ENCODINGS: &[&str] = &["none", "identity", "deflate"];
const CONTENT_LANGUAGES: &[&str] = &["none", "en"];

const VALID_UTF8: &[&[u8]] = &[
    b"a",
    b"Z",
    &[0x00],
    &[0x7f],
    &[0xc2, 0x80],
    &[0xdf, 0xbf],
    &[0xe0, 0xa0, 0x80],
    &[0xed, 0x9f, 0xbf],
    &[0xee, 0x80, 0x80],
    &[0xef, 0xbf, 0xbf],
    &[0xf0, 0x90, 0x80, 0x80],
    &[0xf4, 0x8f, 0xbf, 0xbf],
];
const INVALID_UTF8: &[&[u8]] = &[
    &[0x80],
    &[0xbf],
    &[0xc0, 0x80],
    &[0xc1, 0xbf],
    &[0xc2],
    &[0xe0, 0x80, 0x80],
    &[0xe0, 0x9f, 0xbf],
    &[0xed, 0xa0, 0x80],
    &[0xed, 0xbf, 0xbf],
    &[0xe2, 0x82],
    &[0xf0, 0x80, 0x80, 0x80],
    &[0xf0, 0x8f, 0xbf, 0xbf],
    &[0xf0, 0x9f, 0x98],
    &[0xf4, 0x90, 0x80, 0x80],
    &[0xf5, 0x80, 0x80, 0x80],
    &[0xf8],
    &[0xfe],
    &[0xff],
];

pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64, index: u64) -> Self {
        let mut rng = Rng(seed ^ index.wrapping_mul(0xd1b54a32d192ed03));
        rng.next();
        rng
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }

    fn pick<T: Copy>(&mut self, xs: &[T]) -> T {
        xs[self.below(xs.len() as u64) as usize]
    }
}

struct Gen {
    rng: Rng,
    /// 0 for a document built only from well-formed forms, 1 for about one
    /// fault in it, 4 for several.
    faults: u64,
    out: Vec<u8>,
    magics: Vec<u64>,
    preimages: Vec<(u64, Vec<u8>)>,
}

impl Gen {
    fn fault(&mut self, percent: u64) -> bool {
        self.rng.below(400) < percent * self.faults
    }

    fn byte(&mut self, b: u8) {
        self.out.push(b);
    }

    fn random_byte(&mut self) {
        let b = self.rng.next() as u8;
        self.byte(b);
    }

    fn arg(&mut self) -> u64 {
        match self.rng.below(16) {
            0 => 0,
            1 => 23,
            2 => 24,
            3 => 255,
            4 => 256,
            5 => 65535,
            6 => 65536,
            7 => 0xffff_ffff,
            8 => 0x1_0000_0000,
            9 => 1 << 63,
            10 => u64::MAX,
            11 => self.rng.next(),
            _ => self.rng.below(30),
        }
    }

    /// Any head width the argument fits, minimal most often, or a reserved or
    /// indefinite additional info in its place as a fault.
    fn head(&mut self, major: u8, arg: u64) {
        let m = major << 5;
        if self.fault(2) {
            let info = 28 + self.rng.below(4) as u8;
            self.byte(m | info);
            return;
        }
        let mut widths = vec![];
        if arg < 24 {
            widths.push(0);
        }
        if arg < 1 << 8 {
            widths.push(1);
        }
        if arg < 1 << 16 {
            widths.push(2);
        }
        if arg < 1 << 32 {
            widths.push(4);
        }
        widths.push(8);
        let width = if self.rng.chance(70) {
            widths[0]
        } else {
            self.rng.pick(&widths)
        };
        match width {
            0 => self.byte(m | arg as u8),
            1 => self.byte(m | 24),
            2 => self.byte(m | 25),
            4 => self.byte(m | 26),
            _ => self.byte(m | 27),
        }
        if width > 0 {
            self.out.extend_from_slice(&arg.to_be_bytes()[8 - width..]);
        }
    }

    fn tags(&mut self) {
        if self.rng.chance(6) {
            for _ in 0..1 + self.rng.below(3) {
                let tag = if self.rng.chance(50) {
                    1 + self.rng.below(5)
                } else {
                    self.arg()
                };
                self.head(6, tag);
            }
        }
        if self.rng.chance(1) {
            for _ in 0..118 + self.rng.below(14) {
                self.byte(0xc1);
            }
        }
    }

    fn utf8(&mut self) -> Vec<u8> {
        let invalid = self.fault(20);
        let mut s = vec![];
        for _ in 0..self.rng.below(8) {
            let set = if invalid && self.rng.chance(30) {
                INVALID_UTF8
            } else {
                VALID_UTF8
            };
            s.extend_from_slice(self.rng.pick(set));
        }
        if invalid && !s.iter().any(|b| *b >= 0x80) {
            s.extend_from_slice(self.rng.pick(INVALID_UTF8));
        }
        s
    }

    fn raw(&mut self) -> Vec<u8> {
        let n = match self.rng.below(10) {
            0 => 0,
            1 => 24 + self.rng.below(300),
            _ => 1 + self.rng.below(20),
        };
        (0..n).map(|_| self.rng.next() as u8).collect()
    }

    /// A definite or chunked byte or text string. Chunk boundaries may split a
    /// UTF-8 sequence; a chunk of the wrong major, nested, or tagged, a wrong
    /// length and a missing break are faults.
    fn string(&mut self, major: u8, content: &[u8]) {
        if !self.rng.chance(25) {
            let n = content.len() as u64;
            let n = if self.fault(3) {
                n + 1 + self.rng.below(3)
            } else {
                n
            };
            self.head(major, n);
            self.out.extend_from_slice(content);
            return;
        }
        self.byte((major << 5) | 31);
        let mut rest = content;
        while !rest.is_empty() || self.rng.chance(10) {
            let n = if rest.is_empty() {
                0
            } else {
                1 + self.rng.below(rest.len() as u64) as usize
            };
            let (chunk, tail) = rest.split_at(n);
            rest = tail;
            if self.fault(8) {
                match self.rng.below(3) {
                    0 => self.head(if major == 2 { 3 } else { 2 }, n as u64),
                    1 => self.byte((major << 5) | 31),
                    _ => {
                        self.head(6, 1);
                        self.head(major, n as u64);
                    }
                }
            } else {
                self.head(major, n as u64);
            }
            self.out.extend_from_slice(chunk);
        }
        if !self.fault(3) {
            self.byte(0xff);
        }
    }

    fn text(&mut self) -> Vec<u8> {
        let s = self.utf8();
        self.string(3, &s);
        s
    }

    fn scalar(&mut self) {
        if self.fault(8) {
            let b = self.rng.pick(&[0xfc, 0xfd, 0xfe, 0xff, 0x1f, 0x3f, 0xdf, 0xf8]);
            self.byte(b);
            if b == 0xf8 {
                let v = self.rng.below(32) as u8;
                self.byte(v);
            }
            return;
        }
        match self.rng.below(11) {
            0 | 1 => {
                let a = self.arg();
                self.head(0, a)
            }
            2 => {
                let a = self.arg();
                self.head(1, a)
            }
            3 => {
                let r = self.raw();
                self.string(2, &r)
            }
            4 => {
                self.text();
            }
            5 => {
                let b = 0xe0 + self.rng.below(24) as u8;
                self.byte(b)
            }
            6 => {
                self.byte(0xf8);
                let b = 32 + self.rng.below(224) as u8;
                self.byte(b)
            }
            7 => {
                let n = self.rng.below(3);
                self.byte(0xf9 + n as u8);
                for _ in 0..2 << n {
                    self.random_byte();
                }
            }
            _ => {
                let b = 0xf4 + self.rng.below(4) as u8;
                self.byte(b)
            }
        }
    }

    /// Containers and tags nested from 118 to 131 deep around a scalar.
    fn deep(&mut self) {
        let depth = 118 + self.rng.below(14);
        let uniform = self.rng.chance(50);
        let first = self.rng.below(7);
        let mut closers: Vec<&'static [u8]> = vec![];
        for _ in 0..depth {
            let kind = if uniform { first } else { self.rng.below(7) };
            match kind {
                0 => self.byte(0x81),
                1 => {
                    self.byte(0x9f);
                    closers.push(&[0xff]);
                }
                2 => self.out.extend_from_slice(&[0xa1, 0x00]),
                3 => {
                    self.out.extend_from_slice(&[0xbf, 0x00]);
                    closers.push(&[0xff]);
                }
                4 => {
                    self.byte(0xa1);
                    closers.push(&[0x00]);
                }
                5 => self.byte(0xc1),
                _ => self.out.extend_from_slice(&[0xd8, 0xff]),
            }
        }
        self.scalar();
        for c in closers.iter().rev() {
            self.out.extend_from_slice(c);
        }
    }

    fn value(&mut self, depth: u32) {
        if self.rng.chance(2) {
            self.deep();
            return;
        }
        if depth >= 4 || self.rng.chance(50) {
            self.scalar();
            return;
        }
        match self.rng.below(6) {
            0 => {
                let n = self.rng.below(5);
                self.head(4, n);
                for _ in 0..n {
                    self.value(depth + 1);
                }
            }
            1 => {
                self.byte(0x9f);
                for _ in 0..self.rng.below(5) {
                    self.value(depth + 1);
                }
                if !self.fault(5) {
                    self.byte(0xff);
                }
            }
            2 => {
                let n = self.rng.below(4);
                self.head(5, n);
                for _ in 0..n {
                    self.value(depth + 1);
                    self.value(depth + 1);
                }
            }
            3 => {
                self.byte(0xbf);
                for _ in 0..self.rng.below(4) {
                    self.value(depth + 1);
                    if !self.fault(5) {
                        self.value(depth + 1);
                    }
                }
                if !self.fault(5) {
                    self.byte(0xff);
                }
            }
            _ => {
                let tag = self.arg();
                self.head(6, tag);
                self.value(depth + 1);
            }
        }
    }

    fn key(&mut self, key: u64) {
        self.tags();
        if self.fault(3) {
            self.value(3);
        } else {
            self.head(0, key);
        }
    }

    fn payload(&mut self, magic: u64) {
        self.tags();
        let content = if self.rng.chance(20) {
            NAME_PREIMAGE.to_vec()
        } else {
            self.raw()
        };
        match self.rng.below(20) {
            0..=11 => self.string(2, &content),
            12 | 13 => {
                let s = self.text();
                self.preimages.push((magic, s));
                return;
            }
            14..=17 => {
                let indefinite = self.rng.chance(40);
                if indefinite {
                    self.byte(0x9f);
                } else {
                    self.head(4, content.len() as u64);
                }
                for b in &content {
                    self.tags();
                    if self.fault(8) {
                        match self.rng.below(3) {
                            0 => {
                                let v = 256 + self.rng.below(1000);
                                self.head(0, v)
                            }
                            1 => self.head(1, *b as u64),
                            _ => self.byte(0xf6),
                        }
                    } else {
                        self.head(0, *b as u64);
                    }
                }
                if indefinite && !self.fault(3) {
                    self.byte(0xff);
                }
            }
            _ => self.value(0),
        }
        self.preimages.push((magic, content));
    }

    fn magic_value(&mut self) -> u64 {
        match self.rng.below(25) {
            0 => PREFIX,
            1 => self.rng.next(),
            2 => 0xff00_0000_0000_0000 | self.rng.next() >> 8,
            _ => {
                let magics = std::mem::take(&mut self.magics);
                let m = self.rng.pick(&magics);
                self.magics = magics;
                m
            }
        }
    }

    fn magic(&mut self, magic: u64) {
        self.tags();
        if self.fault(10) {
            match self.rng.below(3) {
                0 => self.head(1, magic),
                1 => self.scalar(),
                _ => {
                    let a = self.arg();
                    self.head(0, a)
                }
            }
        } else {
            self.head(0, magic);
        }
    }

    fn name(&mut self, names: &[&str]) -> Vec<u8> {
        let n = self.rng.pick(names);
        if !self.fault(40) {
            return n.as_bytes().to_vec();
        }
        match self.rng.below(4) {
            0 => n.to_uppercase().into_bytes(),
            1 => format!("{n}x").into_bytes(),
            2 => vec![],
            _ => self.rng.pick(CONTENT_TYPES).as_bytes().to_vec(),
        }
    }

    fn identifier(&mut self, names: &[&str]) {
        self.tags();
        if self.rng.chance(30) {
            let i = if self.fault(30) {
                names.len() as u64 + self.rng.below(2)
            } else {
                self.rng.below(names.len() as u64)
            };
            self.head(0, i);
        } else {
            let n = self.name(names);
            self.string(3, &n);
        }
    }

    fn unit(&mut self) {
        self.tags();
        if self.fault(20) {
            self.scalar();
        } else {
            let b = self.rng.pick(&[0xf6, 0xf7]);
            self.byte(b);
        }
    }

    fn variant(&mut self, names: &[&str]) {
        self.tags();
        if self.fault(5) {
            self.value(0);
            return;
        }
        match self.rng.below(10) {
            0..=4 => self.identifier(names),
            5 | 6 => {
                let indefinite = self.fault(10);
                if indefinite {
                    self.byte(0xbf);
                } else {
                    let n = if self.fault(10) { 2 } else { 1 };
                    self.head(5, n);
                }
                self.identifier(names);
                self.unit();
                if indefinite {
                    self.byte(0xff);
                }
            }
            _ => {
                let indefinite = self.rng.chance(30);
                let n = if self.fault(20) {
                    self.rng.pick(&[0, 1, 3])
                } else {
                    2
                };
                if indefinite {
                    self.byte(0x9f);
                } else {
                    self.head(4, n);
                }
                for i in 0..n {
                    if i == 0 {
                        self.identifier(names);
                    } else {
                        self.unit();
                    }
                }
                if indefinite && !self.fault(5) {
                    self.byte(0xff);
                }
            }
        }
    }

    fn entry(&mut self, key: u64, magic: u64) {
        self.key(key);
        match key {
            0 => self.payload(magic),
            1 => self.magic(magic),
            2 => self.variant(CONTENT_TYPES),
            3 => self.variant(CONTENT_ENCODINGS),
            4 => self.variant(CONTENT_LANGUAGES),
            OA_SCHEMA if !self.fault(25) => {
                self.text();
            }
            _ => self.value(0),
        }
    }

    fn item(&mut self) {
        self.tags();
        let magic = self.magic_value();
        let mut order: Vec<u64> = vec![];
        if !self.fault(10) {
            order.push(1);
        }
        if !self.fault(10) {
            order.push(0);
        }
        for (key, percent) in [(2, 35), (3, 35), (4, 25), (OA_SCHEMA, 8)] {
            if self.rng.chance(percent) {
                order.push(key);
            }
        }
        for _ in 0..self.rng.below(4) {
            let key = match self.rng.below(5) {
                0 => self.arg(),
                1 => 5 + self.rng.below(1 << 20),
                _ => 5 + self.rng.below(20),
            };
            order.push(key);
        }
        if !order.is_empty() && self.fault(8) {
            let dup = self.rng.pick(&order);
            order.push(dup);
        }
        for i in 1..order.len() {
            if self.rng.chance(30) {
                let j = self.rng.below(i as u64 + 1) as usize;
                order.swap(i, j);
            }
        }
        let indefinite = self.rng.chance(15);
        if indefinite {
            self.byte(0xbf);
        } else {
            let n = order.len() as u64;
            let n = if self.fault(6) {
                if n > 0 && self.rng.chance(50) {
                    n - 1
                } else {
                    n + 1
                }
            } else {
                n
            };
            self.head(5, n);
        }
        for key in order {
            self.entry(key, magic);
        }
        if indefinite && !self.fault(4) {
            self.byte(0xff);
        }
    }

    fn document(&mut self) {
        if self.fault(10) {
            match self.rng.below(3) {
                0 => {
                    let r = self.raw();
                    self.out.extend_from_slice(&r[..r.len().min(12)]);
                    return;
                }
                1 => {
                    let mut p = PREFIX.to_be_bytes().to_vec();
                    let i = self.rng.below(8) as usize;
                    p[i] ^= 1 << self.rng.below(8);
                    self.out.extend_from_slice(&p);
                }
                _ => {
                    let n = self.rng.below(8) as usize;
                    self.out.extend_from_slice(&PREFIX.to_be_bytes()[..n]);
                    return;
                }
            }
        } else {
            self.out.extend_from_slice(&PREFIX.to_be_bytes());
        }
        let items = match self.rng.below(10) {
            0 if self.faults > 0 => 0,
            0..=5 => 1,
            6 | 7 => 2,
            _ => 3 + self.rng.below(3),
        };
        for _ in 0..items {
            if self.fault(4) {
                self.value(0);
            } else {
                self.item();
            }
        }
        if self.fault(3) {
            self.scalar();
        }
        if self.out.len() > 8 && self.fault(15) {
            let at = 8 + self.rng.below(self.out.len() as u64 - 8) as usize;
            match self.rng.below(4) {
                0 => self.out.truncate(at),
                1 => self.out[at] ^= 1 << self.rng.below(8),
                2 => {
                    let b = self.rng.next() as u8;
                    self.out.insert(at, b)
                }
                _ => {
                    self.out.remove(at);
                }
            }
        }
    }
}

/// Document `index` of the stream `seed`, and the preimage checks run on it:
/// mostly a magic it holds with the keccak256 of a payload it was built from.
pub fn document(seed: u64, index: u64) -> (Vec<u8>, Vec<(u64, [u8; 32])>) {
    let magics: Vec<u64> = KnownMagic::iter()
        .map(|m| m as u64)
        .filter(|m| KnownMagic::try_from(*m).is_ok())
        .collect();
    let mut rng = Rng::new(seed, index);
    let faults = [0, 0, 0, 0, 1, 1, 1, 1, 4, 4][rng.below(10) as usize];
    let mut g = Gen {
        rng,
        faults,
        out: vec![],
        magics,
        preimages: vec![],
    };
    g.document();
    let mut checks = vec![];
    for _ in 0..1 + g.rng.below(3) {
        let check = match (g.rng.below(10), g.preimages.is_empty()) {
            (0..=6, false) => {
                let (magic, preimage) = g.preimages[g.rng.below(g.preimages.len() as u64) as usize].clone();
                (magic, keccak256(&preimage).0)
            }
            (7 | 8, false) => {
                let magic = g.preimages[g.rng.below(g.preimages.len() as u64) as usize].0;
                let r = g.raw();
                (magic, keccak256(&r).0)
            }
            _ => {
                let magic = g.rng.pick(&g.magics);
                (magic, keccak256(NAME_PREIMAGE).0)
            }
        };
        checks.push(check);
    }
    (g.out, checks)
}
