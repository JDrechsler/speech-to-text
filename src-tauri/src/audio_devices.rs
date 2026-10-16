use std::mem::size_of;
use std::ptr::null;

use core_foundation::base::TCFType;
use core_foundation::string::{CFString, CFStringRef};
use coreaudio_sys::{
    AudioObjectGetPropertyData, AudioObjectGetPropertyDataSize, AudioObjectID, AudioObjectPropertyAddress,
    kAudioDevicePropertyStreams, kAudioHardwarePropertyDevices, kAudioObjectPropertyElementMain,
    kAudioObjectPropertyName, kAudioObjectPropertyScopeGlobal, kAudioObjectPropertyScopeInput,
    kAudioObjectSystemObject,
};

pub fn input_device_names() -> Vec<String> {
    device_ids()
        .into_iter()
        .filter(|&device| has_input_streams(device))
        .filter_map(name)
        .collect()
}

fn address(selector: u32, scope: u32) -> AudioObjectPropertyAddress {
    AudioObjectPropertyAddress { mSelector: selector, mScope: scope, mElement: kAudioObjectPropertyElementMain }
}

fn data_size(object: AudioObjectID, address: &AudioObjectPropertyAddress) -> Option<u32> {
    let mut size = 0u32;
    let status = unsafe { AudioObjectGetPropertyDataSize(object, address, 0, null(), &mut size) };
    (status == 0).then_some(size)
}

fn device_ids() -> Vec<AudioObjectID> {
    let address = address(kAudioHardwarePropertyDevices, kAudioObjectPropertyScopeGlobal);
    let Some(mut size) = data_size(kAudioObjectSystemObject, &address) else {
        return Vec::new();
    };
    let mut ids: Vec<AudioObjectID> = vec![0; size as usize / size_of::<AudioObjectID>()];
    let status = unsafe {
        AudioObjectGetPropertyData(kAudioObjectSystemObject, &address, 0, null(), &mut size, ids.as_mut_ptr().cast())
    };
    if status != 0 {
        return Vec::new();
    }
    ids.truncate(size as usize / size_of::<AudioObjectID>());
    ids
}

fn has_input_streams(device: AudioObjectID) -> bool {
    data_size(device, &address(kAudioDevicePropertyStreams, kAudioObjectPropertyScopeInput))
        .is_some_and(|size| size > 0)
}

fn name(device: AudioObjectID) -> Option<String> {
    let address = address(kAudioObjectPropertyName, kAudioObjectPropertyScopeGlobal);
    let mut name: CFStringRef = std::ptr::null();
    let mut size = size_of::<CFStringRef>() as u32;
    let status = unsafe {
        AudioObjectGetPropertyData(device, &address, 0, null(), &mut size, (&raw mut name).cast())
    };
    if status != 0 || name.is_null() {
        return None;
    }
    Some(unsafe { CFString::wrap_under_create_rule(name) }.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_input_devices_without_opening_them() {
        let names = input_device_names();
        assert!(names.iter().all(|name| !name.is_empty()));
    }
}
