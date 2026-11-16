pub fn children(data: &[u8]) -> Vec<([u8; 4], &[u8], usize)> {
    let mut found = Vec::new();
    let mut offset = 0;
    while offset + 8 <= data.len() {
        let size = u32::from_be_bytes(data[offset..offset + 4].try_into().unwrap()) as usize;
        let kind: [u8; 4] = data[offset + 4..offset + 8].try_into().unwrap();
        found.push((kind, &data[offset + 8..offset + size], offset + 8));
        offset += size;
    }
    found
}

const CONTAINERS: [&[u8; 4]; 7] = [b"moov", b"trak", b"mdia", b"minf", b"stbl", b"dinf", b"edts"];

pub fn find<'a>(data: &'a [u8], path: &[&[u8; 4]]) -> Vec<&'a [u8]> {
    match path.split_first() {
        None => vec![data],
        Some((first, rest)) => children(data)
            .into_iter()
            .filter(|(kind, _, _)| &kind == first)
            .flat_map(|(kind, body, _)| match rest.is_empty() || CONTAINERS.contains(&&kind) {
                true => find(body, rest),
                false => vec![],
            })
            .collect(),
    }
}

pub fn u32_at(data: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes(data[offset..offset + 4].try_into().unwrap())
}

pub fn table(full_box: &[u8], width: usize) -> Vec<Vec<u32>> {
    let count = u32_at(full_box, 4) as usize;
    (0..count)
        .map(|row| (0..width).map(|column| u32_at(full_box, 8 + (row * width + column) * 4)).collect())
        .collect()
}
