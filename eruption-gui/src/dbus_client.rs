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

#![allow(dead_code)]

use crate::{constants, util::RGBA};
use dbus::arg::RefArg;
use dbus::blocking::stdintf::org_freedesktop_dbus::PropertiesPropertiesChanged;
use dbus::blocking::Connection;
use glib::clone;
use std::time::Duration;
use std::{path::Path, thread};

/// Messages received via D-Bus
#[derive(Debug)]
pub enum Message {
    /// Slot has been changed
    SlotChanged(usize),

    /// Slot name has been changed
    SlotNamesChanged(Vec<String>),

    /// Profile has been changed
    ProfileChanged(String),

    /// A device has been hotplugged
    DeviceHotplug((u16, u16, bool)),

    /// Brightness has been changed
    BrightnessChanged(i64),

    /// SoundFX has been toggled
    SoundFxChanged(bool),

    /// Process-monitor rules changed
    RulesChanged,
}

type Result<T> = std::result::Result<T, eyre::Error>;

#[derive(Debug, thiserror::Error)]
pub enum DbusClientError {
    #[error("Unknown error: {description}")]
    UnknownError { description: String },

    #[error("Authentication failed: {description}")]
    AuthError { description: String },

    #[error("Method call failed: {description}")]
    MethodFailed { description: String },
}

type CallbackFn = dyn Fn(&gtk::Builder, &Message) -> crate::Result<()>;

/// Spawn a thread that listens for events on D-Bus
pub fn spawn_dbus_event_loop_system(
    builder: &gtk::Builder,
    callback: &'static CallbackFn,
) -> Result<()> {
    let (tx, rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);

    thread::spawn(move || -> Result<()> {
        let conn = Connection::new_system().unwrap();
        let slot_proxy = conn.with_proxy(
            "org.eruption",
            "/org/eruption/slot",
            Duration::from_millis(4000),
        );

        let profile_proxy = conn.with_proxy(
            "org.eruption",
            "/org/eruption/profile",
            Duration::from_millis(4000),
        );

        let config_proxy = conn.with_proxy(
            "org.eruption",
            "/org/eruption/config",
            Duration::from_millis(4000),
        );

        let devices_proxy = conn.with_proxy(
            "org.eruption",
            "/org/eruption/devices",
            Duration::from_millis(4000),
        );

        let _id1 = slot_proxy.match_signal(
            clone!(@strong tx => move |h: slot::OrgEruptionSlotActiveSlotChanged,
                  _: &Connection,
                  _message: &dbus::Message| {
                tx.send(Message::SlotChanged(h.slot as usize)).unwrap();

                true
            }),
        )?;

        let _id1_1 = slot_proxy
            .match_signal(
                clone!(@strong tx => move |h: slot::OrgFreedesktopDBusPropertiesPropertiesChanged,
                      _: &Connection,
                      _message: &dbus::Message| {

                    // slot names have been changed
                    if let Some(args) = h.changed_properties.get("SlotNames") {
                        let slot_names = args.0.as_iter().unwrap().map(|v| v.as_str().unwrap().to_string()).collect::<Vec<String>>();
                        tx.send(Message::SlotNamesChanged(slot_names)).unwrap();
                    }

                    true
                }),
            )?;

        let _id2 = profile_proxy.match_signal(
            clone!(@strong tx => move |h: profile::OrgEruptionProfileActiveProfileChanged,
                  _: &Connection,
                  _message: &dbus::Message| {
                tx.send(Message::ProfileChanged(h.profile_name))
                    .unwrap();

                true
            }),
        )?;

        let _id3 = config_proxy.match_signal(
            clone!(@strong tx => move |h: config::OrgEruptionConfigBrightnessChanged,
                  _: &Connection,
                  _message: &dbus::Message| {
                tx.send(Message::BrightnessChanged(h.brightness))
                    .unwrap();

                true
            }),
        )?;

        let _id3_1 = config_proxy.match_signal(
            clone!(@strong tx => move |h: PropertiesPropertiesChanged,
                  _: &Connection,
                  _message: &dbus::Message| {

                if let Some(brightness) = h.changed_properties.get("Brightness") {
                    let brightness = brightness.0.as_i64().unwrap();

                    tx.send(Message::BrightnessChanged(brightness))
                        .unwrap();
                }

                if let Some(result) = h.changed_properties.get("EnableSfx") {
                    let enabled = result.0.as_u64().unwrap() != 0;

                    tx.send(Message::SoundFxChanged(enabled))
                        .unwrap();
                }

                true
            }),
        )?;

        let _id4 = devices_proxy.match_signal(
            clone!(@strong tx => move |h: devices::OrgEruptionDeviceDeviceHotplug,
                  _: &Connection,
                  _message: &dbus::Message| {

                tx.send(Message::DeviceHotplug(h.device_info))
                    .unwrap();

                true
            }),
        )?;

        let _id4_1 = devices_proxy.match_signal(
            clone!(@strong tx => move |_h: PropertiesPropertiesChanged,
                  _: &Connection,
                  _message: &dbus::Message| {

                true
            }),
        )?;

        loop {
            conn.process(Duration::from_millis(4000))?;
        }
    });

    rx.attach(
        None,
        clone!(@strong builder, @strong callback => @default-return glib::Continue(true), move |event| {
            callback(&builder, &event).unwrap();
            // thread::yield_now();

            glib::Continue(true)
        }),
    );

    Ok(())
}

/// Spawn a thread that listens for events on the D-Bus session bus
pub fn spawn_dbus_event_loop_session(
    builder: &gtk::Builder,
    callback: &'static CallbackFn,
) -> Result<()> {
    // use process_monitor::OrgEruptionProcessMonitorRules;

    let (tx, rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);

    thread::spawn(move || -> Result<()> {
        let conn = Connection::new_session()?;
        let rules_proxy = conn.with_proxy(
            "org.eruption.process_monitor",
            "/org/eruption/process_monitor/rules",
            Duration::from_millis(4000),
        );

        let _id1 = rules_proxy
            .match_signal(
                clone!(@strong tx => move |_h: process_monitor::OrgEruptionProcessMonitorRulesRulesChanged,
                      _: &Connection,
                      _message: &dbus::Message| {

                        tx.send(Message::RulesChanged).unwrap();

                        false
                }),
            )?;

        // let _id1_1 = rules_proxy.match_signal(
        //     move |h: PropertiesPropertiesChanged, _: &Connection, _message: &dbus::Message| {
        //         log::info!("{:?}", h);

        //         true
        //     },
        // )?;

        loop {
            conn.process(Duration::from_millis(4000))?;
        }
    });

    rx.attach(
        None,
        clone!(@strong builder, @strong callback => @default-return glib::Continue(true), move |event| {
            callback(&builder, &event).unwrap();
            // thread::yield_now();

            glib::Continue(true)
        }),
    );

    Ok(())
}

/// Instruct the daemon to write a .profile file or a Lua script or manifest
pub fn write_file<P: AsRef<Path>>(path: &P, data: &String) -> Result<()> {
    use self::config::OrgEruptionConfig;

    let conn = Connection::new_system()?;
    let proxy = conn.with_proxy(
        "org.eruption",
        "/org/eruption/config",
        Duration::from_secs(35),
    );

    if let Err(e) = proxy.write_file(&*path.as_ref().to_string_lossy(), &data) {
        log::error!("{}", e);

        Err(DbusClientError::MethodFailed {
            description: format!("{}", e),
        }
        .into())
    } else {
        Ok(())
    }
}

pub fn ping() -> Result<()> {
    use self::config::OrgEruptionConfig;

    let conn = Connection::new_system()?;
    let proxy = conn.with_proxy(
        "org.eruption",
        "/org/eruption/config",
        Duration::from_secs(35),
    );

    if let Err(e) = proxy.ping() {
        log::error!("{}", e);

        Err(DbusClientError::MethodFailed {
            description: format!("{}", e),
        }
        .into())
    } else {
        Ok(())
    }
}

pub fn ping_privileged() -> Result<()> {
    use self::config::OrgEruptionConfig;

    let conn = Connection::new_system()?;
    let proxy = conn.with_proxy(
        "org.eruption",
        "/org/eruption/config",
        Duration::from_secs(35),
    );

    if let Err(e) = proxy.ping_privileged() {
        log::error!("{}", e);

        Err(DbusClientError::MethodFailed {
            description: format!("{}", e),
        }
        .into())
    } else {
        Ok(())
    }
}

pub fn set_parameter(
    profile_file: &str,
    script_file: &str,
    param_name: &str,
    value: &str,
) -> Result<()> {
    use profile::OrgEruptionProfile;

    let conn = Connection::new_system()?;
    let proxy = conn.with_proxy(
        "org.eruption",
        "/org/eruption/profile",
        Duration::from_secs(constants::DBUS_TIMEOUT_MILLIS as u64),
    );

    let _result = proxy.set_parameter(&profile_file, &script_file, &param_name, &value)?;

    Ok(())
}

// TODO: This currently fails with a dbus error, use util::get_slot_names() for now
/// Fetches all slot names
// pub fn get_slot_names() -> Result<Vec<String>> {
//     use slot::OrgEruptionSlot;

//     let conn = Connection::new_system()?;
//     let slot_proxy = conn.with_proxy(
//         "org.eruption",
//         "/org/eruption/slots",
//         Duration::from_secs(constants::DBUS_TIMEOUT_MILLIS as u64),
//     );

//     let result = slot_proxy.slot_names()?;

//     Ok(result)
// }

pub fn enumerate_process_monitor_rules() -> Result<Vec<(String, String, String, String)>> {
    use process_monitor::OrgEruptionProcessMonitorRules;

    let conn = Connection::new_session()?;
    let proxy = conn.with_proxy(
        "org.eruption.process_monitor",
        "/org/eruption/process_monitor/rules",
        Duration::from_secs(constants::DBUS_TIMEOUT_MILLIS as u64),
    );

    let result = proxy.enum_rules()?;

    Ok(result)
}

pub fn transmit_process_monitor_rules(rules: &[(&str, &str, &str, &str)]) -> Result<()> {
    use process_monitor::OrgEruptionProcessMonitorRules;

    let conn = Connection::new_session()?;
    let proxy = conn.with_proxy(
        "org.eruption.process_monitor",
        "/org/eruption/process_monitor/rules",
        Duration::from_secs(constants::DBUS_TIMEOUT_MILLIS as u64),
    );

    proxy.set_rules(rules.to_vec())?;

    Ok(())
}

/// Fetches all LED color values from the eruption daemon
pub fn get_led_colors() -> Result<Vec<RGBA>> {
    use status::OrgEruptionStatus;

    let conn = Connection::new_system()?;
    let status_proxy = conn.with_proxy(
        "org.eruption",
        "/org/eruption/status",
        Duration::from_secs(constants::DBUS_TIMEOUT_MILLIS as u64),
    );

    let result = status_proxy.get_led_colors()?;

    let result = result
        .iter()
        .map(|v| RGBA {
            r: v.0,
            g: v.1,
            b: v.2,
            a: v.3,
        })
        .collect::<Vec<RGBA>>();

    Ok(result)
}

/// Get managed devices USB IDs from the eruption daemon
pub fn get_managed_devices() -> Result<(Vec<(u16, u16)>, Vec<(u16, u16)>, Vec<(u16, u16)>)> {
    use status::OrgEruptionStatus;

    let conn = Connection::new_system()?;
    let status_proxy = conn.with_proxy(
        "org.eruption",
        "/org/eruption/status",
        Duration::from_secs(constants::DBUS_TIMEOUT_MILLIS as u64),
    );

    let result = status_proxy.get_managed_devices()?;

    Ok(result)
}

mod slot {
    // This code was autogenerated with `dbus-codegen-rust -s -d org.eruption -p /org/eruption/slot -m None`, see https://github.com/diwic/dbus-rs
    use dbus;
    #[allow(unused_imports)]
    use dbus::arg;
    use dbus::blocking;

    pub trait OrgEruptionSlot {
        fn get_slot_profiles(&self) -> Result<Vec<String>, dbus::Error>;
        fn switch_slot(&self, slot: u64) -> Result<bool, dbus::Error>;
        fn active_slot(&self) -> Result<u64, dbus::Error>;
        fn slot_names(&self) -> Result<Vec<String>, dbus::Error>;
        fn set_slot_names(&self, value: Vec<String>) -> Result<(), dbus::Error>;
    }

    #[derive(Debug)]
    pub struct OrgEruptionSlotActiveSlotChanged {
        pub slot: u64,
    }

    impl arg::AppendAll for OrgEruptionSlotActiveSlotChanged {
        fn append(&self, i: &mut arg::IterAppend) {
            arg::RefArg::append(&self.slot, i);
        }
    }

    impl arg::ReadAll for OrgEruptionSlotActiveSlotChanged {
        fn read(i: &mut arg::Iter) -> Result<Self, arg::TypeMismatchError> {
            Ok(OrgEruptionSlotActiveSlotChanged { slot: i.read()? })
        }
    }

    impl dbus::message::SignalArgs for OrgEruptionSlotActiveSlotChanged {
        const NAME: &'static str = "ActiveSlotChanged";
        const INTERFACE: &'static str = "org.eruption.Slot";
    }

    impl<'a, T: blocking::BlockingSender, C: ::std::ops::Deref<Target = T>> OrgEruptionSlot
        for blocking::Proxy<'a, C>
    {
        fn get_slot_profiles(&self) -> Result<Vec<String>, dbus::Error> {
            self.method_call("org.eruption.Slot", "GetSlotProfiles", ())
                .and_then(|r: (Vec<String>,)| Ok(r.0))
        }

        fn switch_slot(&self, slot: u64) -> Result<bool, dbus::Error> {
            self.method_call("org.eruption.Slot", "SwitchSlot", (slot,))
                .and_then(|r: (bool,)| Ok(r.0))
        }

        fn active_slot(&self) -> Result<u64, dbus::Error> {
            <Self as blocking::stdintf::org_freedesktop_dbus::Properties>::get(
                &self,
                "org.eruption.Slot",
                "ActiveSlot",
            )
        }

        fn slot_names(&self) -> Result<Vec<String>, dbus::Error> {
            <Self as blocking::stdintf::org_freedesktop_dbus::Properties>::get(
                &self,
                "org.eruption.Slot",
                "SlotNames",
            )
        }

        fn set_slot_names(&self, value: Vec<String>) -> Result<(), dbus::Error> {
            <Self as blocking::stdintf::org_freedesktop_dbus::Properties>::set(
                &self,
                "org.eruption.Slot",
                "SlotNames",
                value,
            )
        }
    }

    pub trait OrgFreedesktopDBusIntrospectable {
        fn introspect(&self) -> Result<String, dbus::Error>;
    }

    impl<'a, T: blocking::BlockingSender, C: ::std::ops::Deref<Target = T>>
        OrgFreedesktopDBusIntrospectable for blocking::Proxy<'a, C>
    {
        fn introspect(&self) -> Result<String, dbus::Error> {
            self.method_call("org.freedesktop.DBus.Introspectable", "Introspect", ())
                .and_then(|r: (String,)| Ok(r.0))
        }
    }

    pub trait OrgFreedesktopDBusProperties {
        fn get(
            &self,
            interface_name: &str,
            property_name: &str,
        ) -> Result<arg::Variant<Box<dyn arg::RefArg + 'static>>, dbus::Error>;
        fn get_all(&self, interface_name: &str) -> Result<arg::PropMap, dbus::Error>;
        fn set(
            &self,
            interface_name: &str,
            property_name: &str,
            value: arg::Variant<Box<dyn arg::RefArg>>,
        ) -> Result<(), dbus::Error>;
    }

    #[derive(Debug)]
    pub struct OrgFreedesktopDBusPropertiesPropertiesChanged {
        pub interface_name: String,
        pub changed_properties: arg::PropMap,
        pub invalidated_properties: Vec<String>,
    }

    impl arg::AppendAll for OrgFreedesktopDBusPropertiesPropertiesChanged {
        fn append(&self, i: &mut arg::IterAppend) {
            arg::RefArg::append(&self.interface_name, i);
            arg::RefArg::append(&self.changed_properties, i);
            arg::RefArg::append(&self.invalidated_properties, i);
        }
    }

    impl arg::ReadAll for OrgFreedesktopDBusPropertiesPropertiesChanged {
        fn read(i: &mut arg::Iter) -> Result<Self, arg::TypeMismatchError> {
            Ok(OrgFreedesktopDBusPropertiesPropertiesChanged {
                interface_name: i.read()?,
                changed_properties: i.read()?,
                invalidated_properties: i.read()?,
            })
        }
    }

    impl dbus::message::SignalArgs for OrgFreedesktopDBusPropertiesPropertiesChanged {
        const NAME: &'static str = "PropertiesChanged";
        const INTERFACE: &'static str = "org.freedesktop.DBus.Properties";
    }

    impl<'a, T: blocking::BlockingSender, C: ::std::ops::Deref<Target = T>>
        OrgFreedesktopDBusProperties for blocking::Proxy<'a, C>
    {
        fn get(
            &self,
            interface_name: &str,
            property_name: &str,
        ) -> Result<arg::Variant<Box<dyn arg::RefArg + 'static>>, dbus::Error> {
            self.method_call(
                "org.freedesktop.DBus.Properties",
                "Get",
                (interface_name, property_name),
            )
            .and_then(|r: (arg::Variant<Box<dyn arg::RefArg + 'static>>,)| Ok(r.0))
        }

        fn get_all(&self, interface_name: &str) -> Result<arg::PropMap, dbus::Error> {
            self.method_call(
                "org.freedesktop.DBus.Properties",
                "GetAll",
                (interface_name,),
            )
            .and_then(|r: (arg::PropMap,)| Ok(r.0))
        }

        fn set(
            &self,
            interface_name: &str,
            property_name: &str,
            value: arg::Variant<Box<dyn arg::RefArg>>,
        ) -> Result<(), dbus::Error> {
            self.method_call(
                "org.freedesktop.DBus.Properties",
                "Set",
                (interface_name, property_name, value),
            )
        }
    }
}

mod profile {
    // This code was autogenerated with `dbus-codegen-rust -s -d org.eruption -p /org/eruption/profile -m None`, see https://github.com/diwic/dbus-rs
    use dbus;
    #[allow(unused_imports)]
    use dbus::arg;
    use dbus::blocking;

    pub trait OrgEruptionProfile {
        fn enum_profiles(&self) -> Result<Vec<(String, String)>, dbus::Error>;
        fn set_parameter(
            &self,
            profile_file: &str,
            script_file: &str,
            param_name: &str,
            value: &str,
        ) -> Result<bool, dbus::Error>;
        fn switch_profile(&self, filename: &str) -> Result<bool, dbus::Error>;
        fn active_profile(&self) -> Result<String, dbus::Error>;
    }

    #[derive(Debug)]
    pub struct OrgEruptionProfileActiveProfileChanged {
        pub profile_name: String,
    }

    impl arg::AppendAll for OrgEruptionProfileActiveProfileChanged {
        fn append(&self, i: &mut arg::IterAppend) {
            arg::RefArg::append(&self.profile_name, i);
        }
    }

    impl arg::ReadAll for OrgEruptionProfileActiveProfileChanged {
        fn read(i: &mut arg::Iter) -> Result<Self, arg::TypeMismatchError> {
            Ok(OrgEruptionProfileActiveProfileChanged {
                profile_name: i.read()?,
            })
        }
    }

    impl dbus::message::SignalArgs for OrgEruptionProfileActiveProfileChanged {
        const NAME: &'static str = "ActiveProfileChanged";
        const INTERFACE: &'static str = "org.eruption.Profile";
    }

    #[derive(Debug)]
    pub struct OrgEruptionProfileProfilesChanged {}

    impl arg::AppendAll for OrgEruptionProfileProfilesChanged {
        fn append(&self, _: &mut arg::IterAppend) {}
    }

    impl arg::ReadAll for OrgEruptionProfileProfilesChanged {
        fn read(_: &mut arg::Iter) -> Result<Self, arg::TypeMismatchError> {
            Ok(OrgEruptionProfileProfilesChanged {})
        }
    }

    impl dbus::message::SignalArgs for OrgEruptionProfileProfilesChanged {
        const NAME: &'static str = "ProfilesChanged";
        const INTERFACE: &'static str = "org.eruption.Profile";
    }

    impl<'a, T: blocking::BlockingSender, C: ::std::ops::Deref<Target = T>> OrgEruptionProfile
        for blocking::Proxy<'a, C>
    {
        fn enum_profiles(&self) -> Result<Vec<(String, String)>, dbus::Error> {
            self.method_call("org.eruption.Profile", "EnumProfiles", ())
                .and_then(|r: (Vec<(String, String)>,)| Ok(r.0))
        }

        fn set_parameter(
            &self,
            profile_file: &str,
            script_file: &str,
            param_name: &str,
            value: &str,
        ) -> Result<bool, dbus::Error> {
            self.method_call(
                "org.eruption.Profile",
                "SetParameter",
                (profile_file, script_file, param_name, value),
            )
            .and_then(|r: (bool,)| Ok(r.0))
        }

        fn switch_profile(&self, filename: &str) -> Result<bool, dbus::Error> {
            self.method_call("org.eruption.Profile", "SwitchProfile", (filename,))
                .and_then(|r: (bool,)| Ok(r.0))
        }

        fn active_profile(&self) -> Result<String, dbus::Error> {
            <Self as blocking::stdintf::org_freedesktop_dbus::Properties>::get(
                &self,
                "org.eruption.Profile",
                "ActiveProfile",
            )
        }
    }

    pub trait OrgFreedesktopDBusIntrospectable {
        fn introspect(&self) -> Result<String, dbus::Error>;
    }

    impl<'a, T: blocking::BlockingSender, C: ::std::ops::Deref<Target = T>>
        OrgFreedesktopDBusIntrospectable for blocking::Proxy<'a, C>
    {
        fn introspect(&self) -> Result<String, dbus::Error> {
            self.method_call("org.freedesktop.DBus.Introspectable", "Introspect", ())
                .and_then(|r: (String,)| Ok(r.0))
        }
    }

    pub trait OrgFreedesktopDBusProperties {
        fn get(
            &self,
            interface_name: &str,
            property_name: &str,
        ) -> Result<arg::Variant<Box<dyn arg::RefArg + 'static>>, dbus::Error>;
        fn get_all(&self, interface_name: &str) -> Result<arg::PropMap, dbus::Error>;
        fn set(
            &self,
            interface_name: &str,
            property_name: &str,
            value: arg::Variant<Box<dyn arg::RefArg>>,
        ) -> Result<(), dbus::Error>;
    }

    #[derive(Debug)]
    pub struct OrgFreedesktopDBusPropertiesPropertiesChanged {
        pub interface_name: String,
        pub changed_properties: arg::PropMap,
        pub invalidated_properties: Vec<String>,
    }

    impl arg::AppendAll for OrgFreedesktopDBusPropertiesPropertiesChanged {
        fn append(&self, i: &mut arg::IterAppend) {
            arg::RefArg::append(&self.interface_name, i);
            arg::RefArg::append(&self.changed_properties, i);
            arg::RefArg::append(&self.invalidated_properties, i);
        }
    }

    impl arg::ReadAll for OrgFreedesktopDBusPropertiesPropertiesChanged {
        fn read(i: &mut arg::Iter) -> Result<Self, arg::TypeMismatchError> {
            Ok(OrgFreedesktopDBusPropertiesPropertiesChanged {
                interface_name: i.read()?,
                changed_properties: i.read()?,
                invalidated_properties: i.read()?,
            })
        }
    }

    impl dbus::message::SignalArgs for OrgFreedesktopDBusPropertiesPropertiesChanged {
        const NAME: &'static str = "PropertiesChanged";
        const INTERFACE: &'static str = "org.freedesktop.DBus.Properties";
    }

    impl<'a, T: blocking::BlockingSender, C: ::std::ops::Deref<Target = T>>
        OrgFreedesktopDBusProperties for blocking::Proxy<'a, C>
    {
        fn get(
            &self,
            interface_name: &str,
            property_name: &str,
        ) -> Result<arg::Variant<Box<dyn arg::RefArg + 'static>>, dbus::Error> {
            self.method_call(
                "org.freedesktop.DBus.Properties",
                "Get",
                (interface_name, property_name),
            )
            .and_then(|r: (arg::Variant<Box<dyn arg::RefArg + 'static>>,)| Ok(r.0))
        }

        fn get_all(&self, interface_name: &str) -> Result<arg::PropMap, dbus::Error> {
            self.method_call(
                "org.freedesktop.DBus.Properties",
                "GetAll",
                (interface_name,),
            )
            .and_then(|r: (arg::PropMap,)| Ok(r.0))
        }

        fn set(
            &self,
            interface_name: &str,
            property_name: &str,
            value: arg::Variant<Box<dyn arg::RefArg>>,
        ) -> Result<(), dbus::Error> {
            self.method_call(
                "org.freedesktop.DBus.Properties",
                "Set",
                (interface_name, property_name, value),
            )
        }
    }
}

mod config {
    // This code was autogenerated with `dbus-codegen-rust -s -d org.eruption -p /org/eruption/config -m None`, see https://github.com/diwic/dbus-rs
    use dbus;
    #[allow(unused_imports)]
    use dbus::arg;
    use dbus::blocking;

    pub trait OrgEruptionConfig {
        fn ping(&self) -> Result<bool, dbus::Error>;
        fn ping_privileged(&self) -> Result<bool, dbus::Error>;
        fn write_file(&self, filename: &str, data: &str) -> Result<bool, dbus::Error>;
        fn brightness(&self) -> Result<i64, dbus::Error>;
        fn set_brightness(&self, value: i64) -> Result<(), dbus::Error>;
        fn enable_sfx(&self) -> Result<bool, dbus::Error>;
        fn set_enable_sfx(&self, value: bool) -> Result<(), dbus::Error>;
    }

    #[derive(Debug)]
    pub struct OrgEruptionConfigBrightnessChanged {
        pub brightness: i64,
    }

    impl arg::AppendAll for OrgEruptionConfigBrightnessChanged {
        fn append(&self, i: &mut arg::IterAppend) {
            arg::RefArg::append(&self.brightness, i);
        }
    }

    impl arg::ReadAll for OrgEruptionConfigBrightnessChanged {
        fn read(i: &mut arg::Iter) -> Result<Self, arg::TypeMismatchError> {
            Ok(OrgEruptionConfigBrightnessChanged {
                brightness: i.read()?,
            })
        }
    }

    impl dbus::message::SignalArgs for OrgEruptionConfigBrightnessChanged {
        const NAME: &'static str = "BrightnessChanged";
        const INTERFACE: &'static str = "org.eruption.Config";
    }

    impl<'a, T: blocking::BlockingSender, C: ::std::ops::Deref<Target = T>> OrgEruptionConfig
        for blocking::Proxy<'a, C>
    {
        fn ping(&self) -> Result<bool, dbus::Error> {
            self.method_call("org.eruption.Config", "Ping", ())
                .and_then(|r: (bool,)| Ok(r.0))
        }

        fn ping_privileged(&self) -> Result<bool, dbus::Error> {
            self.method_call("org.eruption.Config", "PingPrivileged", ())
                .and_then(|r: (bool,)| Ok(r.0))
        }

        fn write_file(&self, filename: &str, data: &str) -> Result<bool, dbus::Error> {
            self.method_call("org.eruption.Config", "WriteFile", (filename, data))
                .and_then(|r: (bool,)| Ok(r.0))
        }

        fn brightness(&self) -> Result<i64, dbus::Error> {
            <Self as blocking::stdintf::org_freedesktop_dbus::Properties>::get(
                &self,
                "org.eruption.Config",
                "Brightness",
            )
        }

        fn enable_sfx(&self) -> Result<bool, dbus::Error> {
            <Self as blocking::stdintf::org_freedesktop_dbus::Properties>::get(
                &self,
                "org.eruption.Config",
                "EnableSfx",
            )
        }

        fn set_brightness(&self, value: i64) -> Result<(), dbus::Error> {
            <Self as blocking::stdintf::org_freedesktop_dbus::Properties>::set(
                &self,
                "org.eruption.Config",
                "Brightness",
                value,
            )
        }

        fn set_enable_sfx(&self, value: bool) -> Result<(), dbus::Error> {
            <Self as blocking::stdintf::org_freedesktop_dbus::Properties>::set(
                &self,
                "org.eruption.Config",
                "EnableSfx",
                value,
            )
        }
    }

    pub trait OrgFreedesktopDBusIntrospectable {
        fn introspect(&self) -> Result<String, dbus::Error>;
    }

    impl<'a, T: blocking::BlockingSender, C: ::std::ops::Deref<Target = T>>
        OrgFreedesktopDBusIntrospectable for blocking::Proxy<'a, C>
    {
        fn introspect(&self) -> Result<String, dbus::Error> {
            self.method_call("org.freedesktop.DBus.Introspectable", "Introspect", ())
                .and_then(|r: (String,)| Ok(r.0))
        }
    }

    pub trait OrgFreedesktopDBusProperties {
        fn get(
            &self,
            interface_name: &str,
            property_name: &str,
        ) -> Result<arg::Variant<Box<dyn arg::RefArg + 'static>>, dbus::Error>;
        fn get_all(&self, interface_name: &str) -> Result<arg::PropMap, dbus::Error>;
        fn set(
            &self,
            interface_name: &str,
            property_name: &str,
            value: arg::Variant<Box<dyn arg::RefArg>>,
        ) -> Result<(), dbus::Error>;
    }

    #[derive(Debug)]
    pub struct OrgFreedesktopDBusPropertiesPropertiesChanged {
        pub interface_name: String,
        pub changed_properties: arg::PropMap,
        pub invalidated_properties: Vec<String>,
    }

    impl arg::AppendAll for OrgFreedesktopDBusPropertiesPropertiesChanged {
        fn append(&self, i: &mut arg::IterAppend) {
            arg::RefArg::append(&self.interface_name, i);
            arg::RefArg::append(&self.changed_properties, i);
            arg::RefArg::append(&self.invalidated_properties, i);
        }
    }

    impl arg::ReadAll for OrgFreedesktopDBusPropertiesPropertiesChanged {
        fn read(i: &mut arg::Iter) -> Result<Self, arg::TypeMismatchError> {
            Ok(OrgFreedesktopDBusPropertiesPropertiesChanged {
                interface_name: i.read()?,
                changed_properties: i.read()?,
                invalidated_properties: i.read()?,
            })
        }
    }

    impl dbus::message::SignalArgs for OrgFreedesktopDBusPropertiesPropertiesChanged {
        const NAME: &'static str = "PropertiesChanged";
        const INTERFACE: &'static str = "org.freedesktop.DBus.Properties";
    }

    impl<'a, T: blocking::BlockingSender, C: ::std::ops::Deref<Target = T>>
        OrgFreedesktopDBusProperties for blocking::Proxy<'a, C>
    {
        fn get(
            &self,
            interface_name: &str,
            property_name: &str,
        ) -> Result<arg::Variant<Box<dyn arg::RefArg + 'static>>, dbus::Error> {
            self.method_call(
                "org.freedesktop.DBus.Properties",
                "Get",
                (interface_name, property_name),
            )
            .and_then(|r: (arg::Variant<Box<dyn arg::RefArg + 'static>>,)| Ok(r.0))
        }

        fn get_all(&self, interface_name: &str) -> Result<arg::PropMap, dbus::Error> {
            self.method_call(
                "org.freedesktop.DBus.Properties",
                "GetAll",
                (interface_name,),
            )
            .and_then(|r: (arg::PropMap,)| Ok(r.0))
        }

        fn set(
            &self,
            interface_name: &str,
            property_name: &str,
            value: arg::Variant<Box<dyn arg::RefArg>>,
        ) -> Result<(), dbus::Error> {
            self.method_call(
                "org.freedesktop.DBus.Properties",
                "Set",
                (interface_name, property_name, value),
            )
        }
    }
}

mod status {
    // This code was autogenerated with `dbus-codegen-rust -s -d org.eruption -p /org/eruption/status -m None`, see https://github.com/diwic/dbus-rs
    use dbus;
    #[allow(unused_imports)]
    use dbus::arg;
    use dbus::blocking;

    pub trait OrgEruptionStatus {
        fn get_led_colors(&self) -> Result<Vec<(u8, u8, u8, u8)>, dbus::Error>;
        fn get_managed_devices(
            &self,
        ) -> Result<(Vec<(u16, u16)>, Vec<(u16, u16)>, Vec<(u16, u16)>), dbus::Error>;
        fn running(&self) -> Result<bool, dbus::Error>;
    }

    impl<'a, T: blocking::BlockingSender, C: ::std::ops::Deref<Target = T>> OrgEruptionStatus
        for blocking::Proxy<'a, C>
    {
        fn get_led_colors(&self) -> Result<Vec<(u8, u8, u8, u8)>, dbus::Error> {
            self.method_call("org.eruption.Status", "GetLedColors", ())
                .and_then(|r: (Vec<(u8, u8, u8, u8)>,)| Ok(r.0))
        }

        fn get_managed_devices(
            &self,
        ) -> Result<(Vec<(u16, u16)>, Vec<(u16, u16)>, Vec<(u16, u16)>), dbus::Error> {
            self.method_call("org.eruption.Status", "GetManagedDevices", ())
                .and_then(|r: ((Vec<(u16, u16)>, Vec<(u16, u16)>, Vec<(u16, u16)>),)| Ok(r.0))
        }

        fn running(&self) -> Result<bool, dbus::Error> {
            <Self as blocking::stdintf::org_freedesktop_dbus::Properties>::get(
                &self,
                "org.eruption.Status",
                "Running",
            )
        }
    }

    pub trait OrgFreedesktopDBusIntrospectable {
        fn introspect(&self) -> Result<String, dbus::Error>;
    }

    impl<'a, T: blocking::BlockingSender, C: ::std::ops::Deref<Target = T>>
        OrgFreedesktopDBusIntrospectable for blocking::Proxy<'a, C>
    {
        fn introspect(&self) -> Result<String, dbus::Error> {
            self.method_call("org.freedesktop.DBus.Introspectable", "Introspect", ())
                .and_then(|r: (String,)| Ok(r.0))
        }
    }

    pub trait OrgFreedesktopDBusProperties {
        fn get(
            &self,
            interface_name: &str,
            property_name: &str,
        ) -> Result<arg::Variant<Box<dyn arg::RefArg + 'static>>, dbus::Error>;
        fn get_all(&self, interface_name: &str) -> Result<arg::PropMap, dbus::Error>;
        fn set(
            &self,
            interface_name: &str,
            property_name: &str,
            value: arg::Variant<Box<dyn arg::RefArg>>,
        ) -> Result<(), dbus::Error>;
    }

    #[derive(Debug)]
    pub struct OrgFreedesktopDBusPropertiesPropertiesChanged {
        pub interface_name: String,
        pub changed_properties: arg::PropMap,
        pub invalidated_properties: Vec<String>,
    }

    impl arg::AppendAll for OrgFreedesktopDBusPropertiesPropertiesChanged {
        fn append(&self, i: &mut arg::IterAppend) {
            arg::RefArg::append(&self.interface_name, i);
            arg::RefArg::append(&self.changed_properties, i);
            arg::RefArg::append(&self.invalidated_properties, i);
        }
    }

    impl arg::ReadAll for OrgFreedesktopDBusPropertiesPropertiesChanged {
        fn read(i: &mut arg::Iter) -> Result<Self, arg::TypeMismatchError> {
            Ok(OrgFreedesktopDBusPropertiesPropertiesChanged {
                interface_name: i.read()?,
                changed_properties: i.read()?,
                invalidated_properties: i.read()?,
            })
        }
    }

    impl dbus::message::SignalArgs for OrgFreedesktopDBusPropertiesPropertiesChanged {
        const NAME: &'static str = "PropertiesChanged";
        const INTERFACE: &'static str = "org.freedesktop.DBus.Properties";
    }

    impl<'a, T: blocking::BlockingSender, C: ::std::ops::Deref<Target = T>>
        OrgFreedesktopDBusProperties for blocking::Proxy<'a, C>
    {
        fn get(
            &self,
            interface_name: &str,
            property_name: &str,
        ) -> Result<arg::Variant<Box<dyn arg::RefArg + 'static>>, dbus::Error> {
            self.method_call(
                "org.freedesktop.DBus.Properties",
                "Get",
                (interface_name, property_name),
            )
            .and_then(|r: (arg::Variant<Box<dyn arg::RefArg + 'static>>,)| Ok(r.0))
        }

        fn get_all(&self, interface_name: &str) -> Result<arg::PropMap, dbus::Error> {
            self.method_call(
                "org.freedesktop.DBus.Properties",
                "GetAll",
                (interface_name,),
            )
            .and_then(|r: (arg::PropMap,)| Ok(r.0))
        }

        fn set(
            &self,
            interface_name: &str,
            property_name: &str,
            value: arg::Variant<Box<dyn arg::RefArg>>,
        ) -> Result<(), dbus::Error> {
            self.method_call(
                "org.freedesktop.DBus.Properties",
                "Set",
                (interface_name, property_name, value),
            )
        }
    }
}

mod devices {
    // This code was autogenerated with `dbus-codegen-rust -s -d org.eruption -p /org/eruption/devices -m None`, see https://github.com/diwic/dbus-rs
    use dbus;
    #[allow(unused_imports)]
    use dbus::arg;
    use dbus::blocking;

    pub trait OrgEruptionDevice {
        fn get_device_config(&self, device: u64, param: &str) -> Result<String, dbus::Error>;
        fn get_device_status(&self, device: u64) -> Result<String, dbus::Error>;
        fn get_managed_devices(
            &self,
        ) -> Result<(Vec<(u16, u16)>, Vec<(u16, u16)>, Vec<(u16, u16)>), dbus::Error>;
        fn set_device_config(
            &self,
            device: u64,
            param: &str,
            value: &str,
        ) -> Result<bool, dbus::Error>;
        fn device_status(&self) -> Result<String, dbus::Error>;
    }

    #[derive(Debug)]
    pub struct OrgEruptionDeviceDeviceHotplug {
        pub device_info: (u16, u16, bool),
    }

    impl arg::AppendAll for OrgEruptionDeviceDeviceHotplug {
        fn append(&self, i: &mut arg::IterAppend) {
            arg::RefArg::append(&self.device_info, i);
        }
    }

    impl arg::ReadAll for OrgEruptionDeviceDeviceHotplug {
        fn read(i: &mut arg::Iter) -> Result<Self, arg::TypeMismatchError> {
            Ok(OrgEruptionDeviceDeviceHotplug {
                device_info: i.read()?,
            })
        }
    }

    impl dbus::message::SignalArgs for OrgEruptionDeviceDeviceHotplug {
        const NAME: &'static str = "DeviceHotplug";
        const INTERFACE: &'static str = "org.eruption.Device";
    }

    #[derive(Debug)]
    pub struct OrgEruptionDeviceDeviceStatusChanged {
        pub status: String,
    }

    impl arg::AppendAll for OrgEruptionDeviceDeviceStatusChanged {
        fn append(&self, i: &mut arg::IterAppend) {
            arg::RefArg::append(&self.status, i);
        }
    }

    impl arg::ReadAll for OrgEruptionDeviceDeviceStatusChanged {
        fn read(i: &mut arg::Iter) -> Result<Self, arg::TypeMismatchError> {
            Ok(OrgEruptionDeviceDeviceStatusChanged { status: i.read()? })
        }
    }

    impl dbus::message::SignalArgs for OrgEruptionDeviceDeviceStatusChanged {
        const NAME: &'static str = "DeviceStatusChanged";
        const INTERFACE: &'static str = "org.eruption.Device";
    }

    impl<'a, T: blocking::BlockingSender, C: ::std::ops::Deref<Target = T>> OrgEruptionDevice
        for blocking::Proxy<'a, C>
    {
        fn get_device_config(&self, device: u64, param: &str) -> Result<String, dbus::Error> {
            self.method_call("org.eruption.Device", "GetDeviceConfig", (device, param))
                .and_then(|r: (String,)| Ok(r.0))
        }

        fn get_device_status(&self, device: u64) -> Result<String, dbus::Error> {
            self.method_call("org.eruption.Device", "GetDeviceStatus", (device,))
                .and_then(|r: (String,)| Ok(r.0))
        }

        fn get_managed_devices(
            &self,
        ) -> Result<(Vec<(u16, u16)>, Vec<(u16, u16)>, Vec<(u16, u16)>), dbus::Error> {
            self.method_call("org.eruption.Device", "GetManagedDevices", ())
                .and_then(|r: ((Vec<(u16, u16)>, Vec<(u16, u16)>, Vec<(u16, u16)>),)| Ok(r.0))
        }

        fn set_device_config(
            &self,
            device: u64,
            param: &str,
            value: &str,
        ) -> Result<bool, dbus::Error> {
            self.method_call(
                "org.eruption.Device",
                "SetDeviceConfig",
                (device, param, value),
            )
            .and_then(|r: (bool,)| Ok(r.0))
        }

        fn device_status(&self) -> Result<String, dbus::Error> {
            <Self as blocking::stdintf::org_freedesktop_dbus::Properties>::get(
                &self,
                "org.eruption.Device",
                "DeviceStatus",
            )
        }
    }

    pub trait OrgFreedesktopDBusIntrospectable {
        fn introspect(&self) -> Result<String, dbus::Error>;
    }

    impl<'a, T: blocking::BlockingSender, C: ::std::ops::Deref<Target = T>>
        OrgFreedesktopDBusIntrospectable for blocking::Proxy<'a, C>
    {
        fn introspect(&self) -> Result<String, dbus::Error> {
            self.method_call("org.freedesktop.DBus.Introspectable", "Introspect", ())
                .and_then(|r: (String,)| Ok(r.0))
        }
    }

    pub trait OrgFreedesktopDBusProperties {
        fn get(
            &self,
            interface_name: &str,
            property_name: &str,
        ) -> Result<arg::Variant<Box<dyn arg::RefArg + 'static>>, dbus::Error>;
        fn get_all(&self, interface_name: &str) -> Result<arg::PropMap, dbus::Error>;
        fn set(
            &self,
            interface_name: &str,
            property_name: &str,
            value: arg::Variant<Box<dyn arg::RefArg>>,
        ) -> Result<(), dbus::Error>;
    }

    #[derive(Debug)]
    pub struct OrgFreedesktopDBusPropertiesPropertiesChanged {
        pub interface_name: String,
        pub changed_properties: arg::PropMap,
        pub invalidated_properties: Vec<String>,
    }

    impl arg::AppendAll for OrgFreedesktopDBusPropertiesPropertiesChanged {
        fn append(&self, i: &mut arg::IterAppend) {
            arg::RefArg::append(&self.interface_name, i);
            arg::RefArg::append(&self.changed_properties, i);
            arg::RefArg::append(&self.invalidated_properties, i);
        }
    }

    impl arg::ReadAll for OrgFreedesktopDBusPropertiesPropertiesChanged {
        fn read(i: &mut arg::Iter) -> Result<Self, arg::TypeMismatchError> {
            Ok(OrgFreedesktopDBusPropertiesPropertiesChanged {
                interface_name: i.read()?,
                changed_properties: i.read()?,
                invalidated_properties: i.read()?,
            })
        }
    }

    impl dbus::message::SignalArgs for OrgFreedesktopDBusPropertiesPropertiesChanged {
        const NAME: &'static str = "PropertiesChanged";
        const INTERFACE: &'static str = "org.freedesktop.DBus.Properties";
    }

    impl<'a, T: blocking::BlockingSender, C: ::std::ops::Deref<Target = T>>
        OrgFreedesktopDBusProperties for blocking::Proxy<'a, C>
    {
        fn get(
            &self,
            interface_name: &str,
            property_name: &str,
        ) -> Result<arg::Variant<Box<dyn arg::RefArg + 'static>>, dbus::Error> {
            self.method_call(
                "org.freedesktop.DBus.Properties",
                "Get",
                (interface_name, property_name),
            )
            .and_then(|r: (arg::Variant<Box<dyn arg::RefArg + 'static>>,)| Ok(r.0))
        }

        fn get_all(&self, interface_name: &str) -> Result<arg::PropMap, dbus::Error> {
            self.method_call(
                "org.freedesktop.DBus.Properties",
                "GetAll",
                (interface_name,),
            )
            .and_then(|r: (arg::PropMap,)| Ok(r.0))
        }

        fn set(
            &self,
            interface_name: &str,
            property_name: &str,
            value: arg::Variant<Box<dyn arg::RefArg>>,
        ) -> Result<(), dbus::Error> {
            self.method_call(
                "org.freedesktop.DBus.Properties",
                "Set",
                (interface_name, property_name, value),
            )
        }
    }
}

mod process_monitor {
    // This code was autogenerated with `dbus-codegen-rust -d org.eruption.process_monitor -p /org/eruption/process_monitor/rules -m None`, see https://github.com/diwic/dbus-rs
    use dbus;
    #[allow(unused_imports)]
    use dbus::arg;
    use dbus::blocking;

    pub trait OrgEruptionProcessMonitorRules {
        fn enum_rules(&self) -> Result<Vec<(String, String, String, String)>, dbus::Error>;
        fn set_rules(&self, rules: Vec<(&str, &str, &str, &str)>) -> Result<(), dbus::Error>;
    }

    impl<'a, T: blocking::BlockingSender, C: ::std::ops::Deref<Target = T>>
        OrgEruptionProcessMonitorRules for blocking::Proxy<'a, C>
    {
        fn enum_rules(&self) -> Result<Vec<(String, String, String, String)>, dbus::Error> {
            self.method_call("org.eruption.process_monitor.Rules", "EnumRules", ())
                .and_then(|r: (Vec<(String, String, String, String)>,)| Ok(r.0))
        }

        fn set_rules(&self, rules: Vec<(&str, &str, &str, &str)>) -> Result<(), dbus::Error> {
            self.method_call("org.eruption.process_monitor.Rules", "SetRules", (rules,))
        }
    }

    #[derive(Debug)]
    pub struct OrgEruptionProcessMonitorRulesRulesChanged {
        pub rules: Vec<(String, String, String, String)>,
    }

    impl arg::AppendAll for OrgEruptionProcessMonitorRulesRulesChanged {
        fn append(&self, i: &mut arg::IterAppend) {
            arg::RefArg::append(&self.rules, i);
        }
    }

    impl arg::ReadAll for OrgEruptionProcessMonitorRulesRulesChanged {
        fn read(i: &mut arg::Iter) -> Result<Self, arg::TypeMismatchError> {
            Ok(OrgEruptionProcessMonitorRulesRulesChanged { rules: i.read()? })
        }
    }

    impl dbus::message::SignalArgs for OrgEruptionProcessMonitorRulesRulesChanged {
        const NAME: &'static str = "RulesChanged";
        const INTERFACE: &'static str = "org.eruption.process_monitor.Rules";
    }

    pub trait OrgFreedesktopDBusIntrospectable {
        fn introspect(&self) -> Result<String, dbus::Error>;
    }

    impl<'a, T: blocking::BlockingSender, C: ::std::ops::Deref<Target = T>>
        OrgFreedesktopDBusIntrospectable for blocking::Proxy<'a, C>
    {
        fn introspect(&self) -> Result<String, dbus::Error> {
            self.method_call("org.freedesktop.DBus.Introspectable", "Introspect", ())
                .and_then(|r: (String,)| Ok(r.0))
        }
    }
}
