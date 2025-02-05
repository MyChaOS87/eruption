/*
    This file is part of Eruption.

    Eruption is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    Eruption is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with Eruption.  If not, see <http://www.gnu.org/licenses/>.

    Copyright (c) 2019-2022, The Eruption Development Team
*/

use config::Config;
use lazy_static::lazy_static;
use log::*;
use parking_lot::RwLock;
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use crate::constants;
use crate::plugins::audio;

pub type Result<T> = std::result::Result<T, eyre::Error>;

#[derive(Debug, thiserror::Error)]
pub enum StateError {
    #[error("Could not load global runtime state: {description}")]
    StateLoadError { description: String },

    #[error("Could not save global runtime state: {description}")]
    StateWriteError { description: String },
}

lazy_static! {
    /// Global state
    pub static ref STATE: Arc<RwLock<Option<config::Config>>> = Arc::new(RwLock::new(None));
}

pub type DeviceMakeModelSerial = String;

#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
struct State {
    active_slot: usize,
    slot_names: Vec<String>,
    profiles: Vec<PathBuf>,
    enable_sfx: bool,

    /// Global brightness
    brightness: i64,

    /// Device specific brightness
    device_brightness: HashMap<DeviceMakeModelSerial, i32>,
}

pub fn init_global_runtime_state() -> Result<()> {
    // initialize runtime state to sane defaults
    let mut profiles = crate::SLOT_PROFILES.lock();
    profiles.replace(vec![
        PathBuf::from(constants::DEFAULT_PROFILE_DIR).join("blue-fx-swirl-perlin.profile"),
        PathBuf::from(constants::DEFAULT_PROFILE_DIR).join("red-wave.profile"),
        PathBuf::from(constants::DEFAULT_PROFILE_DIR).join("swirl-perlin.profile"),
        PathBuf::from(constants::DEFAULT_PROFILE_DIR).join("spectrum-analyzer-swirl.profile"),
    ]);

    let default_slot_names = vec![
        "Profile Slot 1".to_string(),
        "Profile Slot 2".to_string(),
        "Profile Slot 3".to_string(),
        "Profile Slot 4".to_string(),
    ];

    let mut slot_names = crate::SLOT_NAMES.lock();
    *slot_names = default_slot_names.clone();

    // load state file
    let state_path = PathBuf::from(constants::STATE_DIR).join("eruption.state");

    let state = Config::builder()
        .add_source(config::File::new(
            state_path.to_str().unwrap(),
            config::FileFormat::Toml,
        ))
        .set_default("active_slot", 0)?
        .set_default("enable_sfx", false)?
        .set_default("brightness", 85)?
        .build()
        .map_err(|e| StateError::StateLoadError {
            description: format!("{}", e),
        })?;

    *STATE.write() = Some(state);

    audio::ENABLE_SFX.store(
        STATE
            .read()
            .as_ref()
            .unwrap()
            .get_bool("enable_sfx")
            .unwrap(),
        Ordering::SeqCst,
    );

    STATE
        .read()
        .as_ref()
        .unwrap()
        .get("profiles")
        .map(|p| {
            profiles.replace(p);
        })
        .unwrap_or_else(|_| warn!("Invalid saved state: profiles"));

    crate::ACTIVE_SLOT.store(
        STATE
            .read()
            .as_ref()
            .unwrap()
            .get::<usize>("active_slot")
            .unwrap() as usize,
        Ordering::SeqCst,
    );

    crate::BRIGHTNESS.store(
        STATE
            .read()
            .as_ref()
            .unwrap()
            .get::<i64>("brightness")
            .unwrap() as isize,
        Ordering::SeqCst,
    );

    *slot_names = STATE
        .read()
        .as_ref()
        .unwrap()
        .get::<Vec<String>>("slot_names")
        .unwrap_or_else(|_| default_slot_names.clone());

    if slot_names.is_empty() || slot_names.len() < constants::NUM_SLOTS {
        *slot_names = default_slot_names;
    }

    perform_sanity_checks();

    Ok(())
}

pub fn init_global_runtime_state_late() -> Result<()> {
    // TODO: retain inactive device's brightness values across
    //       restarts of the Eruption daemon

    let empty = Config::builder().build()?;

    if let Ok(device_brightness) = STATE
        .read()
        .as_ref()
        .unwrap_or(&empty)
        .get_table("device_brightness")
    {
        for device in &*crate::KEYBOARD_DEVICES.read() {
            let make = format!("0x{:x}", device.read().get_usb_vid());
            let model = format!("0x{:x}", device.read().get_usb_pid());
            let serial = device.read().get_serial().unwrap_or("").to_string();

            let val = config::Value::new(None, 100);

            let brightness = device_brightness
                .get(&format!("{}:{}:{}", make, model, serial))
                .unwrap_or(&val);

            let brightness = brightness.clone().into_int().unwrap_or(100) as i32;

            debug!("{}:{}:{} Brightness: {}", make, model, serial, brightness);

            device.write().set_local_brightness(brightness)?;
        }

        for device in &*crate::MOUSE_DEVICES.read() {
            let make = format!("0x{:x}", device.read().get_usb_vid());
            let model = format!("0x{:x}", device.read().get_usb_pid());
            let serial = device.read().get_serial().unwrap_or("").to_string();

            let val = config::Value::new(None, 100);

            let brightness = device_brightness
                .get(&format!("{}:{}:{}", make, model, serial))
                .unwrap_or(&val);

            let brightness = brightness.clone().into_int().unwrap_or(100) as i32;

            debug!("{}:{}:{} Brightness: {}", make, model, serial, brightness);

            device.write().set_local_brightness(brightness)?;
        }

        for device in &*crate::MISC_DEVICES.read() {
            let make = format!("0x{:x}", device.read().get_usb_vid());
            let model = format!("0x{:x}", device.read().get_usb_pid());
            let serial = device.read().get_serial().unwrap_or("").to_string();

            let val = config::Value::new(None, 100);

            let brightness = device_brightness
                .get(&format!("{}:{}:{}", make, model, serial))
                .unwrap_or(&val);

            let brightness = brightness.clone().into_int().unwrap_or(100) as i32;

            debug!("{}:{}:{} Brightness: {}", make, model, serial, brightness);

            device.write().set_local_brightness(brightness)?;
        }
    }

    Ok(())
}

pub fn save_runtime_state() -> Result<()> {
    let state_path = PathBuf::from(constants::STATE_DIR).join("eruption.state");

    let mut device_brightness = HashMap::new();

    for device in &*crate::KEYBOARD_DEVICES.read() {
        let make = format!("0x{:x}", device.read().get_usb_vid());
        let model = format!("0x{:x}", device.read().get_usb_pid());
        let serial = device.read().get_serial().unwrap_or("").to_string();

        let brightness = device.read().get_local_brightness()?;

        debug!("{}:{}:{} Brightness: {}", make, model, serial, brightness);

        device_brightness.insert(format!("{}:{}:{}", make, model, serial), brightness);
    }

    for device in &*crate::MOUSE_DEVICES.read() {
        let make = format!("0x{:x}", device.read().get_usb_vid());
        let model = format!("0x{:x}", device.read().get_usb_pid());
        let serial = device.read().get_serial().unwrap_or("").to_string();

        let brightness = device.read().get_local_brightness()?;

        debug!("{}:{}:{} Brightness: {}", make, model, serial, brightness);

        device_brightness.insert(format!("{}:{}:{}", make, model, serial), brightness);
    }

    for device in &*crate::MISC_DEVICES.read() {
        let make = format!("0x{:x}", device.read().get_usb_vid());
        let model = format!("0x{:x}", device.read().get_usb_pid());
        let serial = device.read().get_serial().unwrap_or("").to_string();

        let brightness = device.read().get_local_brightness()?;

        debug!("{}:{}:{} Brightness: {}", make, model, serial, brightness);

        device_brightness.insert(format!("{}:{}:{}", make, model, serial), brightness);
    }

    let config = State {
        active_slot: crate::ACTIVE_SLOT.load(Ordering::SeqCst),
        slot_names: crate::SLOT_NAMES.lock().clone(),
        profiles: crate::SLOT_PROFILES.lock().as_ref().unwrap().clone(),
        enable_sfx: audio::ENABLE_SFX.load(Ordering::SeqCst),
        brightness: crate::BRIGHTNESS.load(Ordering::SeqCst) as i64,
        device_brightness: device_brightness,
    };

    let toml = toml::ser::to_string_pretty(&config).map_err(|e| StateError::StateWriteError {
        description: format!("{}", e),
    })?;

    fs::write(&state_path, &toml)?;

    Ok(())
}

fn perform_sanity_checks() {
    if STATE
        .read()
        .as_ref()
        .unwrap()
        .get_int("brightness")
        .unwrap()
        < 25
    {
        warn!("Brightness configuration value is set very low, the LEDs will probably stay dark!");
    }

    let active_slot = STATE
        .read()
        .as_ref()
        .unwrap()
        .get_int("active_slot")
        .unwrap();
    if !(0..=3).contains(&active_slot) {
        warn!("Configuration value is outside of the valid range: active_slot");
    }
}
