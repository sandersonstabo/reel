use windows::{
    Win32::{
        Media::MediaFoundation::{
            ICodecAPI, IMFActivate, IMFTransform, MFT_ENUM_FLAG, MFT_FRIENDLY_NAME_Attribute,
            MFT_REGISTER_TYPE_INFO, MFTEnumEx,
        },
        System::{
            Com::CoTaskMemFree,
            Variant::{VARIANT, VT_UI4},
        },
    },
    core::{GUID, PWSTR, Result},
};

pub fn find(category: GUID, flags: MFT_ENUM_FLAG, input: (GUID, GUID), output: (GUID, GUID)) -> Result<Vec<IMFActivate>> {
    let input = MFT_REGISTER_TYPE_INFO {
        guidMajorType: input.0,
        guidSubtype: input.1,
    };
    let output = MFT_REGISTER_TYPE_INFO {
        guidMajorType: output.0,
        guidSubtype: output.1,
    };
    let mut list = std::ptr::null_mut();
    let mut count = 0u32;
    unsafe { MFTEnumEx(category, flags, Some(&input), Some(&output), &mut list, &mut count)? };
    let found = match list.is_null() {
        true => Vec::new(),
        false => unsafe { std::slice::from_raw_parts_mut(list, count as usize) }
            .iter_mut()
            .filter_map(Option::take)
            .collect(),
    };
    unsafe { CoTaskMemFree(Some(list as *const _)) };
    Ok(found)
}

pub fn name(activate: &IMFActivate) -> String {
    let mut raw = PWSTR::null();
    let mut length = 0u32;
    match unsafe { activate.GetAllocatedString(&MFT_FRIENDLY_NAME_Attribute, &mut raw, &mut length) } {
        Ok(()) => {
            let text = unsafe { raw.to_string() }.unwrap_or_default();
            unsafe { CoTaskMemFree(Some(raw.0 as *const _)) };
            text
        }
        Err(_) => "unnamed encoder".to_string(),
    }
}

fn unsigned(value: u32) -> VARIANT {
    let mut variant = VARIANT::default();
    unsafe {
        let inner = &mut *variant.Anonymous.Anonymous;
        inner.vt = VT_UI4;
        inner.Anonymous.ulVal = value;
    }
    variant
}

pub fn configure(codec: &ICodecAPI, settings: &[(GUID, u32)]) -> Vec<GUID> {
    settings
        .iter()
        .filter(|(key, value)| unsafe { codec.SetValue(key, &unsigned(*value)) }.is_err())
        .map(|(key, _)| *key)
        .collect()
}

pub fn activate(activate: &IMFActivate) -> Result<IMFTransform> {
    unsafe { activate.ActivateObject() }
}
