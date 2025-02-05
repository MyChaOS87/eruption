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

use glib::clone;
use gtk::glib;
use gtk::prelude::*;

use crate::constants;
use crate::util;

mod hwdevices;

type Result<T> = std::result::Result<T, eyre::Error>;

// #[derive(Debug, thiserror::Error)]
// pub enum MouseError {
//     #[error("Communication with the Eruption daemon failed")]
//     CommunicationError,
//     // #[error("Invalid layout type specified")]
//     // InvalidLayout,
// }

/// Initialize page "Mouse"
pub fn initialize_mouse_page(
    builder: &gtk::Builder,
    template: &gtk::Builder,
    device: u64,
) -> Result<gtk::Widget> {
    let mouse_device = hwdevices::get_mouse_device(device)?;

    let mouse_device_page = template.object("mouse_device_template").unwrap();

    let notification_box_global: gtk::Box = builder.object("notification_box_global").unwrap();

    let mouse_name_label: gtk::Label = template.object("mouse_device_name_label").unwrap();
    let drawing_area: gtk::DrawingArea = template.object("drawing_area_mouse").unwrap();

    let device_brightness_scale: gtk::Scale = template.object("mouse_brightness_scale").unwrap();

    let mouse_firmware_label: gtk::Label = template.object("mouse_firmware_label").unwrap();
    let mouse_rate_label: gtk::Label = template.object("mouse_rate_label").unwrap();
    let mouse_dpi_label: gtk::Label = template.object("mouse_dpi_label").unwrap();
    let mouse_profile_label: gtk::Label = template.object("mouse_profile_label").unwrap();

    let mouse_signal_label: gtk::Label = template.object("mouse_signal_label").unwrap();
    let signal_strength_progress: gtk::ProgressBar =
        template.object("mouse_signal_strength").unwrap();

    let mouse_battery_level_label: gtk::Label =
        template.object("mouse_battery_level_label").unwrap();
    let battery_level_progress: gtk::ProgressBar = template.object("mouse_battery_level").unwrap();

    let debounce_switch: gtk::Switch = template.object("debounce_switch").unwrap();
    let angle_snapping_switch: gtk::Switch = template.object("angle_snapping_switch").unwrap();

    crate::dbus_client::ping().unwrap_or_else(|_e| {
        notification_box_global.show_now();

        // events::LOST_CONNECTION.store(true, Ordering::SeqCst);
    });

    // device name and status
    let make_and_model = mouse_device.get_make_and_model();
    mouse_name_label.set_label(&format!("{} {}", make_and_model.0, make_and_model.1));

    let mouse_device_handle = mouse_device.get_device();

    let device_brightness = util::get_device_brightness(mouse_device_handle)?;
    device_brightness_scale.set_value(device_brightness as f64);

    device_brightness_scale.connect_value_changed(move |s| {
        // if !events::shall_ignore_pending_ui_event() {
        util::set_device_brightness(mouse_device_handle, s.value() as i64).unwrap();
        // }
    });

    debounce_switch.connect_state_set(move |_s, state| {
        // if !events::shall_ignore_pending_ui_event() {
        util::set_debounce(mouse_device_handle, state).unwrap();
        // }

        gtk::Inhibit(false)
    });

    angle_snapping_switch.connect_state_set(move |_s, state| {
        // if !events::shall_ignore_pending_ui_event() {
        util::set_angle_snapping(mouse_device_handle, state).unwrap();
        // }

        gtk::Inhibit(false)
    });

    // drawing area / mouse indicator
    drawing_area.connect_draw(move |da: &gtk::DrawingArea, context: &cairo::Context| {
        if let Err(_e) = mouse_device.draw_mouse(&da, &context) {
            notification_box_global.show();

            // apparently we have lost the connection to the Eruption daemon
            // events::LOST_CONNECTION.store(true, Ordering::SeqCst);
        } else {
            notification_box_global.hide();

            // if events::LOST_CONNECTION.load(Ordering::SeqCst) {
            //     // we re-established the connection to the Eruption daemon,
            //     // update the GUI to show e.g. newly attached devices
            //     events::LOST_CONNECTION.store(false, Ordering::SeqCst);

            //     events::UPDATE_MAIN_WINDOW.store(true, Ordering::SeqCst);
            // }
        }

        gtk::Inhibit(false)
    });

    // near realtime update path
    crate::register_timer(
        151,
        clone!(@weak signal_strength_progress, @weak battery_level_progress,
                    @weak mouse_signal_label, @weak mouse_battery_level_label =>
                    @default-return Ok(()), move || {

            // device status
            if let Ok(device_status) = util::get_device_status(mouse_device_handle) {
                if let Some(signal_strength_percent) = device_status.get("signal-strength-percent") {
                    let value = signal_strength_percent.parse::<i32>().unwrap_or(0);

                    signal_strength_progress.set_fraction(value as f64 / 100.0);

                    mouse_signal_label.show();
                    signal_strength_progress.show();
                } else {
                    mouse_signal_label.hide();
                    signal_strength_progress.hide();
                }

                if let Some(battery_level_percent) = device_status.get("battery-level-percent") {
                    let value = battery_level_percent.parse::<i32>().unwrap_or(0);

                    battery_level_progress.set_fraction(value as f64 / 100.0);

                    mouse_battery_level_label.show();
                    battery_level_progress.show();
                } else {
                    mouse_battery_level_label.hide();
                    battery_level_progress.hide();
                }
            }

            Ok(())
        }),
    )?;

    // fast update path
    crate::register_timer(
        1051,
        clone!(@weak device_brightness_scale, @weak mouse_dpi_label,
                    @weak mouse_profile_label, @weak debounce_switch,
                    @weak angle_snapping_switch => @default-return Ok(()), move || {

            if let Ok(device_brightness) = util::get_device_brightness(mouse_device_handle) {
                device_brightness_scale.set_value(device_brightness as f64);
            }

            if let Ok(dpi) = util::get_dpi_slot(mouse_device_handle) {
                mouse_dpi_label.set_label(&format!("{}", dpi));
            }

            if let Ok(hardware_profile) = util::get_hardware_profile(mouse_device_handle) {
                mouse_profile_label.set_label(&format!("{}", hardware_profile));
            }

            if let Ok(debounce) = util::get_debounce(mouse_device_handle) {
                debounce_switch.set_active(debounce);
            }

            if let Ok(angle_snapping) = util::get_angle_snapping(mouse_device_handle) {
                angle_snapping_switch.set_active(angle_snapping);
            }

            Ok(())
        }),
    )?;

    // slow update path
    crate::register_timer(
        3023,
        clone!(@weak mouse_firmware_label, @weak mouse_rate_label, @weak signal_strength_progress, @weak battery_level_progress => @default-return Ok(()), move || {
            if let Ok(firmware) = util::get_firmware_revision(mouse_device_handle) {
                mouse_firmware_label.set_label(&firmware);
            }

            if let Ok(poll_rate) = util::get_poll_rate(mouse_device_handle) {
                mouse_rate_label.set_label(&format!("{}", poll_rate));
            }

            Ok(())
        }),
    )?;

    crate::register_timer(
        1000 / constants::TARGET_FPS,
        clone!(@weak drawing_area => @default-return Ok(()), move || {
            drawing_area.queue_draw();

            Ok(())
        }),
    )?;

    Ok(mouse_device_page)
}
