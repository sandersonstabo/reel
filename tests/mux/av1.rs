use reel::mux::av1;

fn obu(kind: u8, payload: &[u8]) -> Vec<u8> {
    [&[(kind << 3) | 0x02, payload.len() as u8][..], payload].concat()
}

const SEQUENCE: &[u8] = &[0b0000_0000, 0b0000_0000, 0b0000_0010, 0b1000_0000];

#[test]
fn parses_sized_obus() {
    let stream = [obu(2, &[]), obu(1, SEQUENCE), obu(6, &[9, 9, 9])].concat();
    let kinds: Vec<u8> = av1::obus(&stream).unwrap().iter().map(|obu| obu.kind).collect();
    assert_eq!(kinds, vec![2, 1, 6]);
}

#[test]
fn rejects_truncated_obus() {
    assert!(av1::obus(&[(6 << 3) | 0x02, 10, 1, 2]).is_none());
}

#[test]
fn samples_drop_temporal_delimiters_and_padding() {
    let stream = [obu(2, &[]), obu(6, &[7]), obu(15, &[0, 0])].concat();
    assert_eq!(av1::to_sample(&stream), obu(6, &[7]));
}

#[test]
fn reads_profile_and_level_from_sequence_header() {
    let header_payload = [0b0010_0000u8, 0, 0, 0b0101_0000];
    let stream = obu(1, &header_payload);
    let sequence = av1::sequence_header(&stream).unwrap();
    let profile = av1::profile(&sequence);
    assert_eq!((profile.profile, profile.level, profile.tier), (1, 10, 0));
}

#[test]
fn config_box_embeds_the_sequence_header() {
    let stream = obu(1, SEQUENCE);
    let config = av1::config_box(&av1::sequence_header(&stream).unwrap());
    assert_eq!(&config[4..8], b"av1C");
    assert_eq!(config[8], 0x81);
    assert_eq!(&config[12..], stream.as_slice());
}

#[test]
fn multi_byte_sizes_are_supported() {
    let payload = vec![5u8; 200];
    let stream = [&[(6 << 3) | 0x02, 0xc8, 0x01][..], &payload].concat();
    assert_eq!(av1::obus(&stream).unwrap()[0].payload.len(), 200);
}
