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

// use crate::manifest;
use crate::{constants, dbus_client, manifest, preferences, profiles};
use byteorder::{ByteOrder, LittleEndian};
use dbus::blocking::stdintf::org_freedesktop_dbus::Properties;
use dbus::blocking::Connection;
// use manifest::Manifest;
// use std::fs;
use crate::manifest::ManifestError;
use lazy_static::lazy_static;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::u8;
use std::{convert::TryFrom, process::Command};
use std::{
    path::{Path, PathBuf},
    process::Child,
    sync::Arc,
};
use std::{thread, time::Duration};

type Result<T> = std::result::Result<T, eyre::Error>;

lazy_static! {
    static ref NETFX_PROCESS_HANDLE: Arc<Mutex<Option<Child>>> = Arc::new(Mutex::new(None));
}

#[derive(Debug, thiserror::Error)]
pub enum UtilError {
    #[error("Process not running")]
    ProcessNotRunning,

    #[error("Daemon restart failed")]
    RestartFailed,
    // #[error("Unknown error: {description}")]
    // UnknownError { description: String },
}

/// Represents an RGBA color value
#[derive(Debug, Copy, Clone)]
pub struct RGBA {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

/// Get RGB components of a 32 bits color value.
// #[allow(clippy::many_single_char_names)]
// pub fn color_to_rgba(c: u32) -> (u8, u8, u8, u8) {
//     let a = u8::try_from((c >> 24) & 0xff).unwrap();
//     let r = u8::try_from((c >> 16) & 0xff).unwrap();
//     let g = u8::try_from((c >> 8) & 0xff).unwrap();
//     let b = u8::try_from(c & 0xff).unwrap();

//     (r, g, b, a)
// }

/// Convert RGBA components to a 32 bits color value.
pub fn gdk_rgba_to_color(color: &gdk::RGBA) -> u32 {
    LittleEndian::read_u32(&[
        (color.blue() * 255.0) as u8,
        (color.green() * 255.0) as u8,
        (color.red() * 255.0) as u8,
        (color.alpha() * 255.0) as u8,
    ])
}

/// Get RGB components of a 32 bits color value.
#[allow(clippy::many_single_char_names)]
pub fn color_to_gdk_rgba(c: u32) -> gdk::RGBA {
    let alpha = u8::try_from((c >> 24) & 0xff).unwrap();
    let red = u8::try_from((c >> 16) & 0xff).unwrap();
    let green = u8::try_from((c >> 8) & 0xff).unwrap();
    let blue = u8::try_from(c & 0xff).unwrap();

    gdk::RGBA::new(red as f64, green as f64, blue as f64, alpha as f64)
}

/// Switch the currently active profile
pub fn switch_profile(name: &str) -> Result<()> {
    let file_name = name.to_owned();

    let conn = Connection::new_system()?;
    let proxy = conn.with_proxy(
        "org.eruption",
        "/org/eruption/profile",
        Duration::from_secs(constants::DBUS_TIMEOUT_MILLIS as u64),
    );

    let (_result,): (bool,) =
        proxy.method_call("org.eruption.Profile", "SwitchProfile", (file_name,))?;

    Ok(())
}

/// Switch the currently active slot
pub fn switch_slot(index: usize) -> Result<()> {
    let conn = Connection::new_system()?;
    let proxy = conn.with_proxy(
        "org.eruption",
        "/org/eruption/slot",
        Duration::from_secs(constants::DBUS_TIMEOUT_MILLIS as u64),
    );

    let (_result,): (bool,) =
        proxy.method_call("org.eruption.Slot", "SwitchSlot", (index as u64,))?;

    Ok(())
}

/// Returns the currently active profile
pub fn get_active_profile() -> Result<String> {
    let conn = Connection::new_system()?;
    let proxy = conn.with_proxy(
        "org.eruption",
        "/org/eruption/profile",
        Duration::from_secs(constants::DBUS_TIMEOUT_MILLIS as u64),
    );

    let result: String = proxy.get("org.eruption.Profile", "ActiveProfile")?;

    Ok(result)
}

/// Returns the currently active slot
pub fn get_active_slot() -> Result<usize> {
    let conn = Connection::new_system()?;
    let proxy = conn.with_proxy(
        "org.eruption",
        "/org/eruption/slot",
        Duration::from_secs(constants::DBUS_TIMEOUT_MILLIS as u64),
    );

    let result: u64 = proxy.get("org.eruption.Slot", "ActiveSlot")?;

    Ok(result as usize)
}

/// Sets a slot name
pub fn set_slot_name(index: usize, name: &str) -> Result<()> {
    let conn = Connection::new_system()?;
    let proxy = conn.with_proxy(
        "org.eruption",
        "/org/eruption/slot",
        Duration::from_secs(constants::DBUS_TIMEOUT_MILLIS as u64),
    );

    let names = get_slot_names()?;
    let mut names = names.iter().map(|v| v.as_ref()).collect::<Vec<_>>();
    names[index] = name;

    let arg = Box::new(names);
    proxy.set("org.eruption.Slot", "SlotNames", arg)?;

    Ok(())
}

/// Sets all slot names at once
// pub fn set_slot_names(names: &[&str]) -> Result<()> {
//     let conn = Connection::new_system()?;
//     let proxy = conn.with_proxy(
//         "org.eruption",
//         "/org/eruption/slot",
//         Duration::from_secs(constants::DBUS_TIMEOUT_MILLIS as u64),
//     );

//     let arg = Box::new(names);
//     proxy.set("org.eruption.Slot", "SlotNames", arg)?;

//     Ok(())
// }

/// Returns all slot names
pub fn get_slot_names() -> Result<Vec<String>> {
    let conn = Connection::new_system()?;
    let proxy = conn.with_proxy(
        "org.eruption",
        "/org/eruption/slot",
        Duration::from_secs(constants::DBUS_TIMEOUT_MILLIS as u64),
    );

    let result: Vec<String> = proxy.get("org.eruption.Slot", "SlotNames")?;

    Ok(result)
}

pub fn get_slot_profiles() -> Result<Vec<String>> {
    let conn = Connection::new_system()?;
    let proxy = conn.with_proxy(
        "org.eruption",
        "/org/eruption/slot",
        Duration::from_secs(constants::DBUS_TIMEOUT_MILLIS as u64),
    );

    let (result,): (Vec<String>,) =
        proxy.method_call("org.eruption.Slot", "GetSlotProfiles", ())?;

    Ok(result)
}

/// Enumerate all available scripts
// pub fn get_script_list() -> Result<Vec<(String, String)>> {
//     let path = constants::DEFAULT_SCRIPT_DIR;
//     let scripts = enumerate_scripts(&path)?;

//     let result = scripts
//         .iter()
//         .map(|s| {
//             (
//                 format!("{} - {}", s.name.clone(), s.description.clone()),
//                 s.script_file.to_string_lossy().to_string(),
//             )
//         })
//         .collect();

//     Ok(result)
// }

// global configuration options

/// Get the current brightness value
pub fn get_brightness() -> Result<i64> {
    let conn = Connection::new_system()?;
    let proxy = conn.with_proxy(
        "org.eruption",
        "/org/eruption/config",
        Duration::from_secs(constants::DBUS_TIMEOUT_MILLIS as u64),
    );

    let result = proxy.get("org.eruption.Config", "Brightness")?;

    Ok(result)
}

/// Set the current brightness value
pub fn set_brightness(brightness: i64) -> Result<()> {
    let conn = Connection::new_system()?;
    let proxy = conn.with_proxy(
        "org.eruption",
        "/org/eruption/config",
        Duration::from_secs(constants::DBUS_TIMEOUT_MILLIS as u64),
    );

    let arg = Box::new(brightness as i64);

    proxy.set("org.eruption.Config", "Brightness", arg)?;

    Ok(())
}

/// Get the device specific status
pub fn get_device_status(device: u64) -> Result<HashMap<String, String>> {
    let conn = Connection::new_system()?;
    let proxy = conn.with_proxy(
        "org.eruption",
        "/org/eruption/devices",
        Duration::from_secs(constants::DBUS_TIMEOUT_MILLIS as u64),
    );

    let (status,): (String,) =
        proxy.method_call("org.eruption.Device", "GetDeviceStatus", (device,))?;

    let result: HashMap<String, String> = serde_json::from_str(&status)?;

    Ok(result)
}

/// Get a device specific config param
pub fn get_device_config(device: u64, param: &str) -> Result<String> {
    let conn = Connection::new_system()?;
    let proxy = conn.with_proxy(
        "org.eruption",
        "/org/eruption/devices",
        Duration::from_secs(constants::DBUS_TIMEOUT_MILLIS as u64),
    );

    let (result,): (String,) = proxy.method_call(
        "org.eruption.Device",
        "GetDeviceConfig",
        (device, param.to_owned()),
    )?;

    Ok(result)
}

/// Set a device specific config param
pub fn set_device_config(device: u64, param: &str, value: &str) -> Result<()> {
    let conn = Connection::new_system()?;
    let proxy = conn.with_proxy(
        "org.eruption",
        "/org/eruption/devices",
        Duration::from_secs(constants::DBUS_TIMEOUT_MILLIS as u64),
    );

    let (_result,): (bool,) = proxy.method_call(
        "org.eruption.Device",
        "SetDeviceConfig",
        (device, param.to_owned(), value.to_owned()),
    )?;

    Ok(())
}

/// Get the current brightness value of device
pub fn get_device_brightness(device: u64) -> Result<i64> {
    let brightness = get_device_config(device, "brightness")?.parse::<i64>()?;

    Ok(brightness)
}

/// Set the current brightness value of device
pub fn set_device_brightness(device: u64, brightness: i64) -> Result<()> {
    set_device_config(device, "brightness", &format!("{}", brightness))?;

    Ok(())
}

pub fn get_firmware_revision(device: u64) -> Result<String> {
    let result = get_device_config(device, "firmware")?;

    Ok(result)
}

pub fn get_poll_rate(device: u64) -> Result<i32> {
    let result = get_device_config(device, "rate")?.parse::<i32>()?;

    Ok(result)
}

pub fn get_dpi_slot(device: u64) -> Result<i32> {
    let result = get_device_config(device, "dpi")?.parse::<i32>()?;

    Ok(result)
}

pub fn get_hardware_profile(device: u64) -> Result<i32> {
    let result = get_device_config(device, "profile")?.parse::<i32>()?;

    Ok(result)
}

pub fn set_debounce(device: u64, debounce: bool) -> Result<()> {
    set_device_config(device, "debounce", if debounce { "true" } else { "false" })?;

    Ok(())
}

pub fn get_debounce(device: u64) -> Result<bool> {
    let result = get_device_config(device, "debounce")?.parse::<bool>()?;

    Ok(result)
}

pub fn set_angle_snapping(device: u64, angle_snapping: bool) -> Result<()> {
    set_device_config(
        device,
        "angle-snapping",
        if angle_snapping { "true" } else { "false" },
    )?;

    Ok(())
}

pub fn get_angle_snapping(device: u64) -> Result<bool> {
    let result = get_device_config(device, "angle-snapping")?.parse::<bool>()?;

    Ok(result)
}

/// Returns true when SoundFX is enabled
pub fn get_sound_fx() -> Result<bool> {
    let conn = Connection::new_system()?;
    let proxy = conn.with_proxy(
        "org.eruption",
        "/org/eruption/config",
        Duration::from_secs(constants::DBUS_TIMEOUT_MILLIS as u64),
    );

    let result = proxy.get("org.eruption.Config", "EnableSfx")?;

    Ok(result)
}

/// Set SoundFX state to `enabled`
pub fn set_sound_fx(enabled: bool) -> Result<()> {
    let conn = Connection::new_system()?;
    let proxy = conn.with_proxy(
        "org.eruption",
        "/org/eruption/config",
        Duration::from_secs(constants::DBUS_TIMEOUT_MILLIS as u64),
    );

    let arg = Box::new(enabled);

    proxy.set("org.eruption.Config", "EnableSfx", arg)?;

    Ok(())
}

#[allow(dead_code)]
pub fn get_script_dirs() -> Vec<PathBuf> {
    let mut result = vec![];

    let config = crate::CONFIG.lock();

    let script_dirs = config
        .as_ref()
        .unwrap()
        .get::<Vec<String>>("global.script_dirs")
        .unwrap_or_else(|_| vec![]);

    let mut script_dirs = script_dirs
        .iter()
        .map(|e| PathBuf::from(e))
        .collect::<Vec<PathBuf>>();

    result.append(&mut script_dirs);

    // if we could not determine a valid set of paths, use a hard coded fallback instead
    if result.is_empty() {
        log::warn!("Using default fallback script directory");

        let path = PathBuf::from(constants::DEFAULT_SCRIPT_DIR);
        result.push(path);
    }

    result
}

/// Returns the absolute path of a script file
pub fn match_script_file(script: &Path) -> Result<PathBuf> {
    let scripts = manifest::get_script_files()?;

    for f in scripts {
        if f.file_name().unwrap_or_default() == script {
            return Ok(f);
        }
    }

    Err(ManifestError::ScriptEnumerationError {}.into())
}

pub fn enumerate_profiles() -> Result<Vec<profiles::Profile>> {
    let mut result = profiles::get_profiles()?;

    // sort profiles by their name
    result.sort_by(|lhs, rhs| lhs.name.cmp(&rhs.name));

    Ok(result)
}

/// Returns the associated manifest path in `PathBuf` for the script `script_path`.
pub fn get_manifest_for(script_file: &Path) -> PathBuf {
    let mut manifest_path = script_file.to_path_buf();
    manifest_path.set_extension("lua.manifest");

    manifest_path
}

pub fn toggle_netfx_ambient(enabled: bool) -> Result<()> {
    // TODO: Make this code more robust
    let (vid, pid) = dbus_client::get_managed_devices()?.0[0];

    let model = format!("{:04x}:{:04x}", vid, pid);
    let host_name = preferences::get_host_name().unwrap_or("localhost".to_string());
    let port_number = preferences::get_port_number()?;

    if enabled {
        switch_profile(
            &Path::join(
                &PathBuf::from(&constants::DEFAULT_PROFILE_DIR),
                "netfx.profile",
            )
            .to_string_lossy()
            .to_string(),
        )?;

        thread::sleep(Duration::from_millis(constants::PROCESS_SPAWN_WAIT_MILLIS));

        let handle = Command::new("/usr/bin/eruption-netfx")
            .arg(&model)
            .arg(&host_name)
            .arg(&format!("{}", port_number))
            .arg(&"ambient")
            .spawn()?;

        *NETFX_PROCESS_HANDLE.lock() = Some(handle);

        Ok(())
    } else {
        if let Some(ref mut handle) = NETFX_PROCESS_HANDLE.lock().as_mut() {
            handle.kill()?;
        } else {
            return Err(UtilError::ProcessNotRunning {}.into());
        }

        *NETFX_PROCESS_HANDLE.lock() = None;

        Ok(())
    }
}

pub fn restart_eruption_daemon() -> Result<()> {
    let status = Command::new("/usr/bin/systemctl")
        .arg("restart")
        .arg("eruption.service")
        .status()?;

    if status.success() {
        Ok(())
    } else {
        Err(UtilError::RestartFailed {}.into())
    }
}

pub fn restart_process_monitor_daemon() -> Result<()> {
    let status = Command::new("/usr/bin/systemctl")
        .arg("--user")
        .arg("restart")
        .arg("eruption-process-monitor.service")
        .status()?;

    if status.success() {
        Ok(())
    } else {
        Err(UtilError::RestartFailed {}.into())
    }
}
