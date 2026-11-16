use super::{annexb, mp4};

const SPS: u8 = 7;
const PPS: u8 = 8;
const IDR: u8 = 5;
const DELIMITER: u8 = 9;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParameterSets {
    pub sps: Vec<u8>,
    pub pps: Vec<u8>,
}

pub fn kind(unit: &[u8]) -> u8 {
    unit.first().map_or(0, |header| header & 0x1f)
}

pub fn is_keyframe(stream: &[u8]) -> bool {
    annexb::units(stream).iter().any(|unit| kind(unit) == IDR)
}

pub fn parameter_sets(stream: &[u8]) -> Option<ParameterSets> {
    let all = annexb::units(stream);
    let find = |wanted: u8| all.iter().find(|unit| kind(unit) == wanted).map(|unit| unit.to_vec());
    Some(ParameterSets {
        sps: find(SPS)?,
        pps: find(PPS)?,
    })
}

pub fn to_sample(stream: &[u8]) -> Vec<u8> {
    annexb::length_prefixed(
        annexb::units(stream)
            .into_iter()
            .filter(|unit| !matches!(kind(unit), SPS | PPS | DELIMITER)),
    )
}

pub fn config_box(parameters: &ParameterSets) -> Vec<u8> {
    let sps = &parameters.sps;
    let pps = &parameters.pps;
    let byte = |index: usize| sps.get(index).copied().unwrap_or(0);
    mp4::boxed(
        b"avcC",
        &[
            &[1, byte(1), byte(2), byte(3), 0xff, 0xe1][..],
            &(sps.len() as u16).to_be_bytes(),
            sps,
            &[1],
            &(pps.len() as u16).to_be_bytes(),
            pps,
        ]
        .concat(),
    )
}
