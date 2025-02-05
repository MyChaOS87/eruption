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

use hidapi::HidApi;
use log::*;
use parking_lot::{Mutex, RwLock};
use std::collections::HashMap;
// use std::sync::atomic::Ordering;
use std::time::Duration;
use std::{any::Any, thread};
use std::{mem::size_of, sync::Arc};

use crate::constants;

use super::{
    DeviceCapabilities, DeviceInfoTrait, DeviceStatus, DeviceTrait, HwDeviceError, MiscDevice,
    MiscDeviceTrait, MouseDeviceTrait, RGBA,
};

pub type Result<T> = super::Result<T>;

// pub const CTRL_INTERFACE: i32 = 0; // Control USB sub device
// pub const LED_INTERFACE: i32 = 5; // LED USB sub device

pub const LED_INTERFACE: i32 = 0; // LED USB sub device

// canvas to LED index mapping
pub const LED_0: usize = constants::CANVAS_SIZE - 36;
pub const LED_1: usize = constants::CANVAS_SIZE - 1;

/// Binds the driver to a device
pub fn bind_hiddev(
    hidapi: &HidApi,
    usb_vid: u16,
    usb_pid: u16,
    serial: &str,
) -> super::Result<MiscDevice> {
    let ctrl_dev = hidapi.device_list().find(|&device| {
        device.vendor_id() == usb_vid
            && device.product_id() == usb_pid
            && device.serial_number().unwrap_or("") == serial
            && device.interface_number() == LED_INTERFACE // CTRL_INTERFACE
    });

    // let led_dev = hidapi.device_list().find(|&device| {
    //     device.vendor_id() == usb_vid
    //         && device.product_id() == usb_pid
    //         && device.serial_number().unwrap_or("") == serial
    //         && device.interface_number() == LED_INTERFACE
    // });

    if ctrl_dev.is_none()
    /*|| led_dev.is_none()*/
    {
        Err(HwDeviceError::EnumerationError {}.into())
    } else {
        Ok(Arc::new(RwLock::new(Box::new(RoccatAimoPad::bind(
            ctrl_dev.unwrap(),
            // led_dev.unwrap(),
        )))))
    }
}

/// ROCCAT Aimo Pad info struct (sent as HID report)
#[derive(Debug, Copy, Clone)]
#[repr(C, packed)]
pub struct DeviceInfo {
    pub report_id: u8,
    pub size: u8,
    pub firmware_version: u8,
    pub reserved1: u8,
    pub reserved2: u8,
    pub reserved3: u8,
}

#[derive(Clone)]
/// Device specific code for the ROCCAT Aimo Pad
pub struct RoccatAimoPad {
    pub is_initialized: bool,

    pub is_bound: bool,
    pub ctrl_hiddev_info: Option<hidapi::DeviceInfo>,
    // pub led_hiddev_info: Option<hidapi::DeviceInfo>,
    pub is_opened: bool,
    pub ctrl_hiddev: Arc<Mutex<Option<hidapi::HidDevice>>>,
    // pub led_hiddev: Arc<Mutex<Option<hidapi::HidDevice>>>,
    pub has_failed: bool,

    // device specific configuration options
    pub brightness: i32,

    // device status
    pub device_status: DeviceStatus,
}

impl RoccatAimoPad {
    /// Binds the driver to the supplied HID device
    pub fn bind(ctrl_dev: &hidapi::DeviceInfo /*, led_dev: &hidapi::DeviceInfo */) -> Self {
        info!("Bound driver: ROCCAT Aimo Pad");

        Self {
            is_initialized: false,

            is_bound: true,
            ctrl_hiddev_info: Some(ctrl_dev.clone()),
            // led_hiddev_info: Some(led_dev.clone()),
            is_opened: false,
            ctrl_hiddev: Arc::new(Mutex::new(None)),
            // led_hiddev: Arc::new(Mutex::new(None)),
            has_failed: false,
            brightness: 100,

            device_status: DeviceStatus(HashMap::new()),
        }
    }

    // pub(self) fn query_ctrl_report(&mut self, id: u8) -> Result<()> {
    //     trace!("Querying control device feature report");
    //
    //     if !self.is_bound {
    //         Err(HwDeviceError::DeviceNotBound {}.into())
    //     } else if !self.is_opened {
    //         Err(HwDeviceError::DeviceNotOpened {}.into())
    //     } else {
    //         match id {
    //             0x0f => {
    //                 let mut buf: [u8; 256] = [0; 256];
    //                 buf[0] = id;
    //
    //                 let ctrl_dev = self.ctrl_hiddev.as_ref().lock();
    //                 let ctrl_dev = ctrl_dev.as_ref().unwrap();
    //
    //                 match ctrl_dev.get_feature_report(&mut buf) {
    //                     Ok(_result) => {
    //                         hexdump::hexdump_iter(&buf).for_each(|s| trace!("  {}", s));
    //
    //                         Ok(())
    //                     }
    //
    //                     Err(_) => Err(HwDeviceError::InvalidResult {}.into()),
    //                 }
    //             }
    //
    //             _ => Err(HwDeviceError::InvalidStatusCode {}.into()),
    //         }
    //     }
    // }

    fn send_ctrl_report(&mut self, id: u8) -> Result<()> {
        trace!("Sending control device feature report");

        if !self.is_bound {
            Err(HwDeviceError::DeviceNotBound {}.into())
        } else if !self.is_opened {
            Err(HwDeviceError::DeviceNotOpened {}.into())
        } else {
            let ctrl_dev = self.ctrl_hiddev.as_ref().lock();
            let ctrl_dev = ctrl_dev.as_ref().unwrap();

            match id {
                0x06 => {
                    let buf: [u8; 96] = [
                        0x06, 0x00, 0x09, 0x07, 0xfa, 0x00, 0xff, 0x00, 0x00, 0xff, 0x09, 0x07,
                        0xfa, 0x00, 0xe6, 0x8c, 0x00, 0xff, 0x80, 0x00, 0x00, 0x09, 0x07, 0xff,
                        0x00, 0xff, 0x00, 0x00, 0xff, 0x09, 0x07, 0xff, 0x00, 0xff, 0x00, 0x00,
                        0xff, 0x00, 0x00, 0x00, 0x09, 0x07, 0xff, 0x00, 0xff, 0x00, 0x00, 0xff,
                        0x09, 0x07, 0xff, 0x00, 0xff, 0x00, 0x00, 0xff, 0x00, 0x00, 0x00, 0x09,
                        0x07, 0xff, 0x00, 0xff, 0x00, 0x00, 0xff, 0x09, 0x07, 0xff, 0x00, 0xff,
                        0x00, 0x00, 0xff, 0x00, 0x00, 0x00, 0x09, 0x07, 0xff, 0x00, 0xff, 0x00,
                        0x00, 0xff, 0x09, 0x07, 0xff, 0x00, 0xff, 0x00, 0x00, 0xff, 0x00, 0x00,
                    ];

                    match ctrl_dev.send_feature_report(&buf) {
                        Ok(_result) => {
                            hexdump::hexdump_iter(&buf).for_each(|s| trace!("  {}", s));
                        }

                        Err(_) => return Err(HwDeviceError::InvalidResult {}.into()),
                    }

                    Ok(())
                }

                0x02 => {
                    let buf: [u8; 19] = [
                        0x02, 0x00, 0x09, 0x07, 0xfa, 0x00, 0xff, 0x00, 0x00, 0xff, 0x09, 0x07,
                        0xfa, 0x00, 0xe6, 0x8c, 0x00, 0xff, 0x80,
                    ];

                    match ctrl_dev.send_feature_report(&buf) {
                        Ok(_result) => {
                            hexdump::hexdump_iter(&buf).for_each(|s| trace!("  {}", s));
                        }

                        Err(_) => return Err(HwDeviceError::InvalidResult {}.into()),
                    }

                    Ok(())
                }

                0x04 => {
                    let buf: [u8; 4] = [0x04, 0x00, 0x00, 0xff];

                    match ctrl_dev.send_feature_report(&buf) {
                        Ok(_result) => {
                            hexdump::hexdump_iter(&buf).for_each(|s| trace!("  {}", s));
                        }

                        Err(_) => return Err(HwDeviceError::InvalidResult {}.into()),
                    };

                    Ok(())
                }

                0x01 => {
                    let buf: [u8; 5] = [0x01, 0xff, 0x00, 0x00, 0x00];

                    match ctrl_dev.send_feature_report(&buf) {
                        Ok(_result) => {
                            hexdump::hexdump_iter(&buf).for_each(|s| trace!("  {}", s));
                        }

                        Err(_) => return Err(HwDeviceError::InvalidResult {}.into()),
                    };

                    Ok(())
                }

                _ => Err(HwDeviceError::InvalidStatusCode {}.into()),
            }
        }
    }

    fn wait_for_ctrl_dev(&mut self) -> Result<()> {
        trace!("Waiting for control device to respond...");

        if !self.is_bound {
            Err(HwDeviceError::DeviceNotBound {}.into())
        } else if !self.is_opened {
            Err(HwDeviceError::DeviceNotOpened {}.into())
        } else {
            // let mut buf: [u8; 24] = [0; 24];
            // buf[0] = 0x00;
            //
            // let ctrl_dev = self.ctrl_hiddev.as_ref().lock();
            // let ctrl_dev = ctrl_dev.as_ref().unwrap();
            //
            // match ctrl_dev.read_timeout(&mut buf, 20) {
            //     Ok(_result) => {
            //         hexdump::hexdump_iter(&buf).for_each(|s| trace!("  {}", s));
            //
            //         #[allow(clippy::if_same_then_else)]
            //         if buf[1] == 0x00 || buf[0..5] == [0xe6, 0x06, 0x03, 0x00, 0x04] {
            //             Ok(())
            //         } else if buf[0..4] == [0xa1, 0x84, 0x06, 0x02] {
            //             Ok(()) // directly after device reset
            //         } else {
            //             hexdump::hexdump_iter(&buf).for_each(|s| debug!("  {}", s));
            //
            //             Err(HwDeviceError::InvalidResult {}.into())
            //         }
            //     }
            //
            //     Err(_) => {
            //         hexdump::hexdump_iter(&buf).for_each(|s| debug!("  {}", s));
            //
            //         Err(HwDeviceError::InvalidResult {}.into())
            //     }
            // }

            thread::sleep(Duration::from_millis(25));

            Ok(())
        }
    }
}

impl DeviceInfoTrait for RoccatAimoPad {
    fn get_device_capabilities(&self) -> DeviceCapabilities {
        DeviceCapabilities {}
    }

    fn get_device_info(&self) -> Result<super::DeviceInfo> {
        trace!("Querying the device for information...");

        if !self.is_bound {
            Err(HwDeviceError::DeviceNotBound {}.into())
        } else if !self.is_opened {
            Err(HwDeviceError::DeviceNotOpened {}.into())
        } else {
            let mut buf = [0; size_of::<DeviceInfo>()];
            buf[0] = 0x09; // Query device info (HID report 0x09)

            let ctrl_dev = self.ctrl_hiddev.as_ref().lock();
            let ctrl_dev = ctrl_dev.as_ref().unwrap();

            match ctrl_dev.get_feature_report(&mut buf) {
                Ok(_result) => {
                    hexdump::hexdump_iter(&buf).for_each(|s| trace!("  {}", s));
                    let tmp: DeviceInfo =
                        unsafe { std::ptr::read_unaligned(buf.as_ptr() as *const _) };

                    let result = super::DeviceInfo::new(tmp.firmware_version as i32);
                    Ok(result)
                }

                Err(_) => Err(HwDeviceError::InvalidResult {}.into()),
            }
        }
    }

    fn get_firmware_revision(&self) -> String {
        // if let Ok(device_info) = self.get_device_info() {
        //     format!(
        //         "{}.{:02}",
        //         device_info.firmware_version / 100,
        //         device_info.firmware_version % 100
        //     )
        // } else {
        "<unknown>".to_string()
        // }
    }
}

impl DeviceTrait for RoccatAimoPad {
    fn get_usb_path(&self) -> String {
        self.ctrl_hiddev_info
            .clone()
            .unwrap()
            .path()
            .to_str()
            .unwrap()
            .to_string()
    }

    fn get_usb_vid(&self) -> u16 {
        self.ctrl_hiddev_info.as_ref().unwrap().vendor_id()
    }

    fn get_usb_pid(&self) -> u16 {
        self.ctrl_hiddev_info.as_ref().unwrap().product_id()
    }

    fn get_serial(&self) -> Option<&str> {
        self.ctrl_hiddev_info.as_ref().unwrap().serial_number()
    }

    fn get_support_script_file(&self) -> String {
        "misc/roccat_aimo_pad".to_string()
    }

    fn open(&mut self, api: &hidapi::HidApi) -> Result<()> {
        trace!("Opening HID devices now...");

        if !self.is_bound {
            Err(HwDeviceError::DeviceNotBound {}.into())
        } else {
            trace!("Opening control device...");

            match self.ctrl_hiddev_info.as_ref().unwrap().open_device(api) {
                Ok(dev) => *self.ctrl_hiddev.lock() = Some(dev),
                Err(_) => return Err(HwDeviceError::DeviceOpenError {}.into()),
            };

            // trace!("Opening LED device...");

            // match self.led_hiddev_info.as_ref().unwrap().open_device(api) {
            //     Ok(dev) => *self.led_hiddev.lock() = Some(dev),
            //     Err(_) => return Err(HwDeviceError::DeviceOpenError {}.into()),
            // };

            self.is_opened = true;

            Ok(())
        }
    }

    fn close_all(&mut self) -> Result<()> {
        trace!("Closing HID devices now...");

        // close keyboard device
        if !self.is_bound {
            Err(HwDeviceError::DeviceNotBound {}.into())
        } else if !self.is_opened {
            Err(HwDeviceError::DeviceNotOpened {}.into())
        } else {
            trace!("Closing control device...");
            *self.ctrl_hiddev.lock() = None;

            // trace!("Closing LED device...");
            // *self.led_hiddev.lock() = None;

            self.is_opened = false;

            Ok(())
        }
    }

    fn send_init_sequence(&mut self) -> Result<()> {
        trace!("Sending device init sequence...");

        if !self.is_bound {
            Err(HwDeviceError::DeviceNotBound {}.into())
        } else if !self.is_opened {
            Err(HwDeviceError::DeviceNotOpened {}.into())
        } else {
            // match self.get_device_info() {
            //     Ok(device_info) => {
            //         if device_info.firmware_version < 116 {
            //             warn!(
            //                 "Outdated firmware version: {}, should be: >= 1.23",
            //                 format!(
            //                     "{}.{:02}",
            //                     device_info.firmware_version / 100,
            //                     device_info.firmware_version % 100
            //                 )
            //             );
            //         }
            //     }

            //     Err(e) => {
            //         error!("Could not get firmware version: {}", e);
            //     }
            // }

            self.send_ctrl_report(0x06)
                .unwrap_or_else(|e| error!("Step 1: {}", e));
            self.wait_for_ctrl_dev()
                .unwrap_or_else(|e| error!("Wait 1: {}", e));

            self.send_ctrl_report(0x02)
                .unwrap_or_else(|e| error!("Step 2: {}", e));
            self.wait_for_ctrl_dev()
                .unwrap_or_else(|e| error!("Wait 2: {}", e));

            self.send_ctrl_report(0x04)
                .unwrap_or_else(|e| error!("Step 3: {}", e));
            self.wait_for_ctrl_dev()
                .unwrap_or_else(|e| error!("Wait 3: {}", e));

            self.send_ctrl_report(0x01)
                .unwrap_or_else(|e| error!("Step 4: {}", e));
            self.wait_for_ctrl_dev()
                .unwrap_or_else(|e| error!("Wait 4: {}", e));

            self.is_initialized = true;

            Ok(())
        }
    }

    fn is_initialized(&self) -> Result<bool> {
        Ok(self.is_initialized)
    }

    fn has_failed(&self) -> Result<bool> {
        Ok(self.has_failed)
    }

    fn write_data_raw(&self, buf: &[u8]) -> Result<()> {
        if !self.is_bound {
            Err(HwDeviceError::DeviceNotBound {}.into())
        } else if !self.is_opened {
            Err(HwDeviceError::DeviceNotOpened {}.into())
        } else if !self.is_initialized {
            Err(HwDeviceError::DeviceNotInitialized {}.into())
        } else {
            let ctrl_dev = self.ctrl_hiddev.as_ref().lock();
            let ctrl_dev = ctrl_dev.as_ref().unwrap();

            match ctrl_dev.write(buf) {
                Ok(_result) => {
                    hexdump::hexdump_iter(buf).for_each(|s| trace!("  {}", s));

                    Ok(())
                }

                Err(_) => Err(HwDeviceError::InvalidResult {}.into()),
            }
        }
    }

    fn read_data_raw(&self, size: usize) -> Result<Vec<u8>> {
        if !self.is_bound {
            Err(HwDeviceError::DeviceNotBound {}.into())
        } else if !self.is_opened {
            Err(HwDeviceError::DeviceNotOpened {}.into())
        } else if !self.is_initialized {
            Err(HwDeviceError::DeviceNotInitialized {}.into())
        } else {
            let ctrl_dev = self.ctrl_hiddev.as_ref().lock();
            let ctrl_dev = ctrl_dev.as_ref().unwrap();

            let mut buf = Vec::new();
            buf.resize(size, 0);

            match ctrl_dev.read(buf.as_mut_slice()) {
                Ok(_result) => {
                    hexdump::hexdump_iter(&buf).for_each(|s| trace!("  {}", s));

                    Ok(buf)
                }

                Err(_) => Err(HwDeviceError::InvalidResult {}.into()),
            }
        }
    }

    fn device_status(&self) -> Result<DeviceStatus> {
        Ok(self.device_status.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn as_device(&self) -> &dyn DeviceTrait {
        self
    }

    fn as_device_mut(&mut self) -> &mut dyn DeviceTrait {
        self
    }

    fn as_mouse_device(&self) -> Option<&dyn MouseDeviceTrait> {
        None
    }

    fn as_mouse_device_mut(&mut self) -> Option<&mut dyn MouseDeviceTrait> {
        None
    }
}

impl MiscDeviceTrait for RoccatAimoPad {
    fn has_input_device(&self) -> bool {
        false
    }

    fn set_local_brightness(&mut self, brightness: i32) -> Result<()> {
        trace!("Setting device specific brightness");

        self.brightness = brightness;

        Ok(())
    }

    fn get_local_brightness(&self) -> Result<i32> {
        trace!("Querying device specific brightness");

        Ok(self.brightness)
    }

    fn send_led_map(&mut self, led_map: &[RGBA]) -> Result<()> {
        trace!("Setting LEDs from supplied map...");

        if !self.is_bound {
            Err(HwDeviceError::DeviceNotBound {}.into())
        } else if !self.is_opened {
            Err(HwDeviceError::DeviceNotOpened {}.into())
        } else if !self.is_initialized {
            Err(HwDeviceError::DeviceNotInitialized {}.into())
        } else {
            let ctrl_dev = self.ctrl_hiddev.as_ref().lock();
            let ctrl_dev = ctrl_dev.as_ref().unwrap();

            let buf: [u8; 9] = [
                0x03,
                (led_map[LED_0].r as f32 * (self.brightness as f32 / 100.0)).round() as u8,
                (led_map[LED_0].g as f32 * (self.brightness as f32 / 100.0)).round() as u8,
                (led_map[LED_0].b as f32 * (self.brightness as f32 / 100.0)).round() as u8,
                0xff - (led_map[LED_0].a as f32 * (self.brightness as f32 / 100.0)).round() as u8,
                (led_map[LED_1].r as f32 * (self.brightness as f32 / 100.0)).round() as u8,
                (led_map[LED_1].g as f32 * (self.brightness as f32 / 100.0)).round() as u8,
                (led_map[LED_1].b as f32 * (self.brightness as f32 / 100.0)).round() as u8,
                0xff - (led_map[LED_1].a as f32 * (self.brightness as f32 / 100.0)).round() as u8,
            ];

            match ctrl_dev.send_feature_report(&buf) {
                Ok(_result) => {
                    hexdump::hexdump_iter(&buf).for_each(|s| trace!("  {}", s));
                }

                Err(_) => {
                    // the device has failed or has been disconnected
                    self.is_initialized = false;
                    self.is_opened = false;
                    self.has_failed = true;

                    return Err(HwDeviceError::InvalidResult {}.into());
                }
            };

            Ok(())
        }
    }

    fn set_led_init_pattern(&mut self) -> Result<()> {
        trace!("Setting LED init pattern...");

        if !self.is_bound {
            Err(HwDeviceError::DeviceNotBound {}.into())
        } else if !self.is_opened {
            Err(HwDeviceError::DeviceNotOpened {}.into())
        } else if !self.is_initialized {
            Err(HwDeviceError::DeviceNotInitialized {}.into())
        } else {
            let led_map: [RGBA; constants::CANVAS_SIZE] = [RGBA {
                r: 0x00,
                g: 0x00,
                b: 0x00,
                a: 0x00,
            }; constants::CANVAS_SIZE];

            self.send_led_map(&led_map)?;

            Ok(())
        }
    }

    fn set_led_off_pattern(&mut self) -> Result<()> {
        trace!("Setting LED off pattern...");

        if !self.is_bound {
            Err(HwDeviceError::DeviceNotBound {}.into())
        } else if !self.is_opened {
            Err(HwDeviceError::DeviceNotOpened {}.into())
        } else if !self.is_initialized {
            Err(HwDeviceError::DeviceNotInitialized {}.into())
        } else {
            let led_map: [RGBA; constants::CANVAS_SIZE] = [RGBA {
                r: 0x00,
                g: 0x00,
                b: 0x00,
                a: 0x00,
            }; constants::CANVAS_SIZE];

            self.send_led_map(&led_map)?;

            Ok(())
        }
    }
}
