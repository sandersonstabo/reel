pub fn units(stream: &[u8]) -> Vec<&[u8]> {
    let starts: Vec<usize> = (0..stream.len().saturating_sub(2))
        .filter(|&index| stream[index..index + 3] == [0, 0, 1])
        .map(|index| index + 3)
        .collect();
    starts
        .iter()
        .enumerate()
        .map(|(position, &start)| {
            let end = starts.get(position + 1).map_or(stream.len(), |next| next - 3);
            let unit = &stream[start..end.max(start)];
            let trimmed = unit.iter().rposition(|byte| *byte != 0).map_or(0, |last| last + 1);
            &unit[..trimmed]
        })
        .filter(|unit| !unit.is_empty())
        .collect()
}

pub fn length_prefixed<'a>(units: impl Iterator<Item = &'a [u8]>) -> Vec<u8> {
    units
        .flat_map(|unit| (unit.len() as u32).to_be_bytes().into_iter().chain(unit.iter().copied()))
        .collect()
}

pub fn unescape(unit: &[u8]) -> Vec<u8> {
    unit.iter()
        .fold((Vec::with_capacity(unit.len()), 0usize), |(mut out, zeros), &byte| match (zeros >= 2, byte) {
            (true, 3) => (out, 0),
            _ => {
                out.push(byte);
                (out, if byte == 0 { zeros + 1 } else { 0 })
            }
        })
        .0
}
