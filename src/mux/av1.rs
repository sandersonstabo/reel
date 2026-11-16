use super::mp4;

const SEQUENCE_HEADER: u8 = 1;
const TEMPORAL_DELIMITER: u8 = 2;
const PADDING: u8 = 15;
const DEFAULT_LEVEL: u8 = 8;
const EIGHT_BIT_420: u8 = 0b0000_1100;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Obu<'a> {
    pub kind: u8,
    pub bytes: &'a [u8],
    pub payload: &'a [u8],
}

fn leb128(data: &[u8]) -> Option<(usize, usize)> {
    data.iter()
        .take(8)
        .enumerate()
        .try_fold(0usize, |value, (index, byte)| {
            let value = value | (usize::from(byte & 0x7f) << (7 * index));
            match byte & 0x80 {
                0 => Err((value, index + 1)),
                _ => Ok(value),
            }
        })
        .err()
}

fn next(data: &[u8]) -> Option<(Obu<'_>, &[u8])> {
    let header = *data.first()?;
    let extension = usize::from(header & 0x04 != 0);
    let header_length = 1 + extension;
    let (size, size_length) = match header & 0x02 {
        0 => (data.len().checked_sub(header_length)?, 0),
        _ => leb128(data.get(header_length..)?)?,
    };
    let start = header_length + size_length;
    let end = start.checked_add(size).filter(|end| *end <= data.len())?;
    Some((
        Obu {
            kind: (header >> 3) & 0x0f,
            bytes: &data[..end],
            payload: &data[start..end],
        },
        &data[end..],
    ))
}

pub fn obus(data: &[u8]) -> Option<Vec<Obu<'_>>> {
    let mut rest = data;
    let mut found = Vec::new();
    while !rest.is_empty() {
        let (obu, remaining) = next(rest)?;
        found.push(obu);
        rest = remaining;
    }
    Some(found)
}

pub fn sequence_header(data: &[u8]) -> Option<Obu<'_>> {
    obus(data)?.into_iter().find(|obu| obu.kind == SEQUENCE_HEADER)
}

pub fn to_sample(data: &[u8]) -> Vec<u8> {
    obus(data)
        .unwrap_or_default()
        .into_iter()
        .filter(|obu| !matches!(obu.kind, TEMPORAL_DELIMITER | PADDING))
        .flat_map(|obu| obu.bytes.iter().copied())
        .collect()
}

struct Bits<'a> {
    data: &'a [u8],
    position: usize,
}

impl Bits<'_> {
    fn read(&mut self, count: usize) -> Option<u32> {
        (0..count).try_fold(0u32, |value, _| {
            let byte = *self.data.get(self.position / 8)?;
            let bit = (byte >> (7 - self.position % 8)) & 1;
            self.position += 1;
            Some((value << 1) | u32::from(bit))
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Profile {
    pub profile: u8,
    pub level: u8,
    pub tier: u8,
}

pub fn profile(sequence: &Obu) -> Profile {
    let mut bits = Bits {
        data: sequence.payload,
        position: 0,
    };
    let profile = bits.read(3).unwrap_or(0) as u8;
    let parsed = (|| {
        let _still = bits.read(1)?;
        let reduced = bits.read(1)?;
        match reduced {
            1 => Some((bits.read(5)? as u8, 0)),
            _ => {
                let timing = bits.read(1)?;
                (timing == 0).then_some(())?;
                let _initial_delay = bits.read(1)?;
                let _count = bits.read(5)?;
                let _idc = bits.read(12)?;
                let level = bits.read(5)? as u8;
                let tier = match level > 7 {
                    true => bits.read(1)? as u8,
                    false => 0,
                };
                Some((level, tier))
            }
        }
    })();
    let (level, tier) = parsed.unwrap_or((DEFAULT_LEVEL, 0));
    Profile { profile, level, tier }
}

pub fn config_box(sequence: &Obu) -> Vec<u8> {
    let Profile { profile, level, tier } = profile(sequence);
    mp4::boxed(
        b"av1C",
        &[
            &[0x81, (profile << 5) | (level & 0x1f), (tier << 7) | EIGHT_BIT_420, 0][..],
            sequence.bytes,
        ]
        .concat(),
    )
}
