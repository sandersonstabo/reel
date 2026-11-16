use std::{env, fs, path::PathBuf};

#[allow(dead_code)]
mod glyph {
    include!("src/tray/glyph.rs");
}

const SIZES: [u32; 7] = [16, 20, 24, 32, 40, 48, 64];
const RT_ICON: u16 = 3;
const RT_GROUP_ICON: u16 = 14;
const RT_MANIFEST: u16 = 24;
const MANIFEST_FLAGS: u16 = 0x1030;
const ICON_FLAGS: u16 = 0x1010;
const ENGLISH: u16 = 0x0409;

fn bitmap(size: u32) -> Vec<u8> {
    let mask_row = size.div_ceil(32) * 4;
    let pixels = glyph::pixels(size, glyph::Shape::Disc, glyph::RED);
    let header = [
        40u32.to_le_bytes(),
        size.to_le_bytes(),
        (size * 2).to_le_bytes(),
    ]
    .concat();
    let rest = [
        &1u16.to_le_bytes()[..],
        &32u16.to_le_bytes(),
        &0u32.to_le_bytes(),
        &(size * size * 4 + mask_row * size).to_le_bytes(),
        &[0u8; 16],
    ]
    .concat();
    let color: Vec<u8> = (0..size)
        .rev()
        .flat_map(|row| pixels[(row * size) as usize..((row + 1) * size) as usize].to_vec())
        .flat_map(|argb| [argb as u8, (argb >> 8) as u8, (argb >> 16) as u8, (argb >> 24) as u8])
        .collect();
    [header, rest, color, vec![0u8; (mask_row * size) as usize]].concat()
}

fn resource(kind: u16, id: u16, flags: u16, data: &[u8]) -> Vec<u8> {
    let header = [
        &(data.len() as u32).to_le_bytes()[..],
        &32u32.to_le_bytes(),
        &0xffffu16.to_le_bytes(),
        &kind.to_le_bytes(),
        &0xffffu16.to_le_bytes(),
        &id.to_le_bytes(),
        &0u32.to_le_bytes(),
        &flags.to_le_bytes(),
        &ENGLISH.to_le_bytes(),
        &0u32.to_le_bytes(),
        &0u32.to_le_bytes(),
    ]
    .concat();
    let padding = (4 - data.len() % 4) % 4;
    [header, data.to_vec(), vec![0u8; padding]].concat()
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/tray/glyph.rs");
    println!("cargo:rerun-if-changed=installer/reel.manifest");
    if !env::var("TARGET").is_ok_and(|target| target.contains("windows-msvc")) {
        return;
    }
    let images: Vec<Vec<u8>> = SIZES.iter().map(|size| bitmap(*size)).collect();
    let empty = [&0u32.to_le_bytes()[..], &32u32.to_le_bytes(), &[0xff, 0xff, 0, 0, 0xff, 0xff, 0, 0], &[0u8; 16]].concat();
    let icons: Vec<u8> = images
        .iter()
        .enumerate()
        .flat_map(|(index, image)| resource(RT_ICON, index as u16 + 1, ICON_FLAGS, image))
        .collect();
    let directory = [
        &0u16.to_le_bytes()[..],
        &1u16.to_le_bytes(),
        &(images.len() as u16).to_le_bytes(),
    ]
    .concat();
    let entries: Vec<u8> = SIZES
        .iter()
        .zip(&images)
        .enumerate()
        .flat_map(|(index, (size, image))| {
            [
                &[*size as u8, *size as u8, 0, 0][..],
                &1u16.to_le_bytes(),
                &32u16.to_le_bytes(),
                &(image.len() as u32).to_le_bytes(),
                &(index as u16 + 1).to_le_bytes(),
            ]
            .concat()
        })
        .collect();
    let group = resource(RT_GROUP_ICON, 1, ICON_FLAGS, &[directory, entries].concat());
    let manifest = resource(
        RT_MANIFEST,
        1,
        MANIFEST_FLAGS,
        &fs::read("installer/reel.manifest").expect("manifest is readable"),
    );
    let output = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR is set by cargo")).join("reel.res");
    fs::write(&output, [empty, icons, group, manifest].concat()).expect("resource file is writable");
    println!("cargo:rustc-link-arg-bins={}", output.display());
}
