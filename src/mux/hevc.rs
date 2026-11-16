use super::{annexb, mp4};

const VPS: u8 = 32;
const SPS: u8 = 33;
const PPS: u8 = 34;
const DELIMITER: u8 = 35;
const PROFILE_TIER_LEVEL: std::ops::Range<usize> = 3..15;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParameterSets {
    pub vps: Vec<u8>,
    pub sps: Vec<u8>,
    pub pps: Vec<u8>,
}

pub fn kind(unit: &[u8]) -> u8 {
    unit.first().map_or(0, |header| (header >> 1) & 0x3f)
}

pub fn is_keyframe(stream: &[u8]) -> bool {
    annexb::units(stream).iter().any(|unit| (16..=21).contains(&kind(unit)))
}

pub fn parameter_sets(stream: &[u8]) -> Option<ParameterSets> {
    let all = annexb::units(stream);
    let find = |wanted: u8| all.iter().find(|unit| kind(unit) == wanted).map(|unit| unit.to_vec());
    Some(ParameterSets {
        vps: find(VPS)?,
        sps: find(SPS)?,
        pps: find(PPS)?,
    })
}

pub fn to_sample(stream: &[u8]) -> Vec<u8> {
    annexb::length_prefixed(
        annexb::units(stream)
            .into_iter()
            .filter(|unit| !matches!(kind(unit), VPS | SPS | PPS | DELIMITER)),
    )
}

fn array(kind: u8, unit: &[u8]) -> Vec<u8> {
    [&[0x80 | kind][..], &1u16.to_be_bytes(), &(unit.len() as u16).to_be_bytes(), unit].concat()
}

pub fn config_box(parameters: &ParameterSets) -> Option<Vec<u8>> {
    let sps = annexb::unescape(&parameters.sps);
    let profile = sps.get(PROFILE_TIER_LEVEL)?;
    let sub_layers = (sps[2] >> 1) & 0x07;
    let nested = sps[2] & 0x01;
    let layering = ((sub_layers + 1) << 3) | (nested << 2) | 0x03;
    Some(mp4::boxed(
        b"hvcC",
        &[
            &[1][..],
            profile,
            &[0xf0, 0x00, 0xfc, 0xfd, 0xf8, 0xf8, 0x00, 0x00, layering, 3],
            &array(VPS, &parameters.vps),
            &array(SPS, &parameters.sps),
            &array(PPS, &parameters.pps),
        ]
        .concat(),
    ))
}
