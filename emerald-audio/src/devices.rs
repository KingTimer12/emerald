//! The microphones and speakers the system offers.

use cpal::traits::{DeviceTrait, HostTrait};

/// One device, as the system names it. `id` is what [`crate::Choice`]
/// takes back; it survives restarts on the same machine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Device {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Devices {
    pub inputs: Vec<Device>,
    pub outputs: Vec<Device>,
}

/// Lists the devices. Slow on some systems (tens of milliseconds), so call
/// it off the UI thread.
pub fn devices() -> Devices {
    let host = cpal::default_host();
    let list = |devices: Option<Vec<cpal::Device>>| -> Vec<Device> {
        devices
            .unwrap_or_default()
            .into_iter()
            .filter_map(|device| {
                let id = device.id().ok()?.to_string();
                let name = device.description().ok()?.name().to_owned();
                Some(Device { id, name })
            })
            .collect()
    };
    Devices {
        inputs: list(host.input_devices().ok().map(Iterator::collect)),
        outputs: list(host.output_devices().ok().map(Iterator::collect)),
    }
}

/// The device with `id`, else the system default.
pub(crate) fn pick(host: &cpal::Host, id: Option<&str>, input: bool) -> Option<cpal::Device> {
    let chosen = id.and_then(|id| id.parse().ok()).and_then(|id| host.device_by_id(&id));
    if chosen.is_none() && id.is_some() {
        tracing::info!(input, "audio: the chosen device is gone, using the default");
    }
    chosen.or_else(|| if input { host.default_input_device() } else { host.default_output_device() })
}
