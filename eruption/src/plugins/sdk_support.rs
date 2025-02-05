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

use crate::{
    constants, hwdevices, init_keyboard_device, init_misc_device, init_mouse_device, script,
    spawn_keyboard_input_thread, spawn_misc_input_thread, spawn_mouse_input_thread, DbusApiEvent,
    SDK_SUPPORT_ACTIVE,
};
use crossbeam::channel::unbounded;
use lazy_static::lazy_static;
use log::{debug, error, info, trace};
use mlua::prelude::*;
use nix::poll::{poll, PollFd, PollFlags};
use nix::unistd::unlink;
use parking_lot::{Mutex, RwLock};
use prost::Message;
use protocol::request::Payload as RequestPayload;
use protocol::response::Payload as ResponsePayload;
use socket2::{Domain, SockAddr, Socket, Type};
use std::any::Any;
use std::io::Cursor;
use std::mem::MaybeUninit;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::io::AsRawFd;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use std::{fs, thread};

use crate::hwdevices::RGBA;
use crate::plugins::{self, Plugin};

pub mod protocol {
    include!(concat!(env!("OUT_DIR"), "/sdk_support.rs"));
}

pub type Result<T> = std::result::Result<T, eyre::Error>;

#[derive(Debug, thiserror::Error)]
pub enum SdkPluginError {
    #[error("Eruption SDK plugin error: {description}")]
    PluginError { description: String },
}

lazy_static! {
    /// Global LED map, the "canvas"
    pub static ref LED_MAP: Arc<RwLock<Vec<RGBA>>> = Arc::new(RwLock::new(vec![RGBA {
        r: 0x00,
        g: 0x00,
        b: 0x00,
        a: 0x00,
    }; constants::CANVAS_SIZE]));
}

lazy_static! {
    pub static ref LISTENER: Arc<Mutex<Option<Socket>>> = Arc::new(Mutex::new(None));
}

use bincode::{Decode, Encode};

#[derive(Debug, Default, Clone, Encode, Decode)]
pub struct HotplugInfo {
    pub usb_vid: u16,
    pub usb_pid: u16,
}

pub fn claim_hotplugged_devices(_hotplug_info: &HotplugInfo) -> Result<()> {
    if crate::QUIT.load(Ordering::SeqCst) {
        info!("Ignoring device hotplug event since Eruption is shutting down");
    } else {
        // enumerate devices
        info!("Enumerating connected devices...");

        if let Ok(devices) = hwdevices::probe_devices_hotplug() {
            // initialize keyboard devices
            for (index, device) in devices.0.iter().enumerate() {
                if !crate::KEYBOARD_DEVICES.read().iter().any(|d| {
                    d.read().get_usb_vid() == device.read().get_usb_vid()
                        && d.read().get_usb_pid() == device.read().get_usb_pid()
                }) {
                    info!("Initializing the hotplugged keyboard device...");

                    init_keyboard_device(device);

                    // place a request to re-enter the main loop, this will drop all global locks
                    crate::REENTER_MAIN_LOOP.store(true, Ordering::SeqCst);
                    thread::sleep(Duration::from_millis(25));

                    let usb_vid = device.read().get_usb_vid();
                    let usb_pid = device.read().get_usb_pid();

                    // spawn a thread to handle keyboard input
                    info!("Spawning keyboard input thread...");

                    let (kbd_tx, kbd_rx) = unbounded();
                    spawn_keyboard_input_thread(
                        kbd_tx.clone(),
                        device.clone(),
                        index,
                        usb_vid,
                        usb_pid,
                    )
                    .unwrap_or_else(|e| {
                        error!("Could not spawn a thread: {}", e);
                        panic!()
                    });

                    crate::KEYBOARD_DEVICES_RX.write().push(kbd_rx);
                    crate::KEYBOARD_DEVICES.write().push(device.clone());

                    debug!("Sending device hotplug notification...");

                    let dbus_api_tx = crate::DBUS_API_TX.lock();
                    let dbus_api_tx = dbus_api_tx.as_ref().unwrap();

                    dbus_api_tx
                        .send(DbusApiEvent::DeviceHotplug((usb_vid, usb_pid), false))
                        .unwrap_or_else(|e| {
                            error!("Could not send a pending dbus API event: {}", e)
                        });
                }
            }

            // initialize mouse devices
            for (index, device) in devices.1.iter().enumerate() {
                let enable_mouse = (*crate::CONFIG.lock())
                    .as_ref()
                    .unwrap()
                    .get::<bool>("global.enable_mouse")
                    .unwrap_or(true);

                // enable mouse input
                if enable_mouse {
                    if !crate::MOUSE_DEVICES.read().iter().any(|d| {
                        d.read().get_usb_vid() == device.read().get_usb_vid()
                            && d.read().get_usb_pid() == device.read().get_usb_pid()
                    }) {
                        info!("Initializing the hotplugged mouse device...");

                        init_mouse_device(device);

                        // place a request to re-enter the main loop, this will drop all global locks
                        crate::REENTER_MAIN_LOOP.store(true, Ordering::SeqCst);
                        thread::sleep(Duration::from_millis(25));

                        let usb_vid = device.read().get_usb_vid();
                        let usb_pid = device.read().get_usb_pid();

                        let (mouse_tx, mouse_rx) = unbounded();
                        // let (mouse_secondary_tx, _mouse_secondary_rx) = unbounded();

                        // spawn a thread to handle mouse input
                        info!("Spawning mouse input thread...");

                        spawn_mouse_input_thread(
                            mouse_tx.clone(),
                            device.clone(),
                            index,
                            usb_vid,
                            usb_pid,
                        )
                        .unwrap_or_else(|e| {
                            error!("Could not spawn a thread: {}", e);
                            panic!()
                        });

                        // spawn a thread to handle possible sub-devices
                        /* if EXPERIMENTAL_FEATURES.load(Ordering::SeqCst)
                            && device.read().has_secondary_device()
                        {
                            info!("Spawning mouse input thread for secondary sub-device...");
                            spawn_mouse_input_thread_secondary(
                                mouse_secondary_tx,
                                device.clone(),
                                index,
                                usb_vid,
                                usb_pid,
                            )
                            .unwrap_or_else(|e| {
                                error!("Could not spawn a thread: {}", e);
                                panic!()
                            });
                        }*/

                        crate::MOUSE_DEVICES_RX.write().push(mouse_rx);
                        crate::MOUSE_DEVICES.write().push(device.clone());

                        debug!("Sending device hotplug notification...");

                        let dbus_api_tx = crate::DBUS_API_TX.lock();
                        let dbus_api_tx = dbus_api_tx.as_ref().unwrap();

                        dbus_api_tx
                            .send(DbusApiEvent::DeviceHotplug((usb_vid, usb_pid), false))
                            .unwrap_or_else(|e| {
                                error!("Could not send a pending dbus API event: {}", e)
                            });
                    }
                } else {
                    info!("Found mouse device, but mouse support is DISABLED by configuration");
                }
            }

            // initialize misc devices
            for (index, device) in devices.2.iter().enumerate() {
                if !crate::MISC_DEVICES.read().iter().any(|d| {
                    d.read().get_usb_vid() == device.read().get_usb_vid()
                        && d.read().get_usb_pid() == device.read().get_usb_pid()
                }) {
                    info!("Initializing the hotplugged misc device...");

                    init_misc_device(device);

                    // place a request to re-enter the main loop, this will drop all global locks
                    crate::REENTER_MAIN_LOOP.store(true, Ordering::SeqCst);
                    thread::sleep(Duration::from_millis(25));

                    if device.read().has_input_device() {
                        let usb_vid = device.read().get_usb_vid();
                        let usb_pid = device.read().get_usb_pid();

                        // spawn a thread to handle keyboard input
                        info!("Spawning misc device input thread...");

                        let (misc_tx, misc_rx) = unbounded();
                        spawn_misc_input_thread(
                            misc_tx.clone(),
                            device.clone(),
                            index,
                            usb_vid,
                            usb_pid,
                        )
                        .unwrap_or_else(|e| {
                            error!("Could not spawn a thread: {}", e);
                            panic!()
                        });

                        crate::MISC_DEVICES_RX.write().push(misc_rx);

                        debug!("Sending device hotplug notification...");

                        let dbus_api_tx = crate::DBUS_API_TX.lock();
                        let dbus_api_tx = dbus_api_tx.as_ref().unwrap();

                        dbus_api_tx
                            .send(DbusApiEvent::DeviceHotplug((usb_vid, usb_pid), false))
                            .unwrap_or_else(|e| {
                                error!("Could not send a pending dbus API event: {}", e)
                            });
                    } else {
                        // insert an unused rx
                        let (_misc_tx, misc_rx) = unbounded();
                        crate::MISC_DEVICES_RX.write().push(misc_rx);

                        debug!("Sending device hotplug notification...");

                        let dbus_api_tx = crate::DBUS_API_TX.lock();
                        let dbus_api_tx = dbus_api_tx.as_ref().unwrap();

                        dbus_api_tx
                            .send(DbusApiEvent::DeviceHotplug((0, 0), false))
                            .unwrap_or_else(|e| {
                                error!("Could not send a pending dbus API event: {}", e)
                            });
                    }

                    crate::MISC_DEVICES.write().push(device.clone());
                }
            }

            info!("Device enumeration completed");
        }
    }

    Ok(())
}

///
pub struct SdkSupportPlugin {}

impl SdkSupportPlugin {
    pub fn new() -> Self {
        SdkSupportPlugin {}
    }

    pub fn initialize_socket() -> Result<()> {
        // unlink any leftover control sockets
        let _result = unlink(constants::CONTROL_SOCKET_NAME)
            .map_err(|e| debug!("Unlink of control socket failed: {}", e));

        // create, bind and store the control socket
        let listener = Socket::new(Domain::UNIX, Type::SEQPACKET, None)?;
        let address = SockAddr::unix(&constants::CONTROL_SOCKET_NAME)?;
        listener.bind(&address)?;

        // set permissions of the control socket, allow only root
        let mut perms = fs::metadata(constants::CONTROL_SOCKET_NAME)?.permissions();
        // perms.set_mode(0o660); // don't allow others, only user and group rw
        perms.set_mode(0o666);
        fs::set_permissions(constants::CONTROL_SOCKET_NAME, perms)?;

        LISTENER.lock().replace(listener);

        Ok(())
    }

    pub fn start_control_thread() -> Result<()> {
        let builder = thread::Builder::new().name("control".into());
        builder
            .spawn(move || loop {
                if crate::QUIT.load(Ordering::SeqCst) {
                    break;
                }

                Self::run_io_loop().unwrap_or_else(|e| {
                    error!("Eruption SDK Plugin thread error: {}", e);
                });
            })
            .unwrap_or_else(|e| {
                error!("Could not spawn a thread: {}", e);
                panic!()
            });

        Ok(())
    }

    pub fn run_io_loop() -> Result<()> {
        unsafe fn assume_init(buf: &[MaybeUninit<u8>]) -> &[u8] {
            &*(buf as *const [MaybeUninit<u8>] as *const [u8])
        }

        'IO_LOOP: loop {
            if crate::QUIT.load(Ordering::SeqCst) {
                break 'IO_LOOP;
            }

            if let Some(listener) = LISTENER.lock().as_ref() {
                listener.listen(1)?;

                match listener.accept() {
                    Ok((socket, _sockaddr)) => {
                        debug!("Eruption SDK client connected");

                        // socket.set_nodelay(true)?; // not supported on AF_UNIX on Linux
                        socket.set_send_buffer_size(constants::NET_BUFFER_CAPACITY * 2)?;
                        socket.set_recv_buffer_size(constants::NET_BUFFER_CAPACITY * 2)?;

                        // connection successful, enter event loop now
                        'EVENT_LOOP: loop {
                            if crate::QUIT.load(Ordering::SeqCst) {
                                break 'EVENT_LOOP;
                            }

                            // wait for socket to be ready
                            let mut poll_fds = [PollFd::new(
                                socket.as_raw_fd(),
                                PollFlags::POLLIN
                                    | PollFlags::POLLOUT
                                    | PollFlags::POLLHUP
                                    | PollFlags::POLLERR,
                            )];

                            let result = poll(&mut poll_fds, constants::SLEEP_TIME_TIMEOUT as i32)?;

                            if poll_fds[0].revents().unwrap().contains(PollFlags::POLLHUP)
                                | poll_fds[0].revents().unwrap().contains(PollFlags::POLLERR)
                            {
                                debug!("Eruption SDK client disconnected");

                                break 'EVENT_LOOP;
                            }

                            if result > 0
                                && poll_fds[0].revents().unwrap().contains(PollFlags::POLLIN)
                            {
                                // read data
                                let mut tmp =
                                    [MaybeUninit::zeroed(); constants::NET_BUFFER_CAPACITY];
                                match socket.recv(&mut tmp) {
                                    Ok(0) => {
                                        debug!("Eruption SDK client disconnected");

                                        break 'EVENT_LOOP;
                                    }

                                    Ok(n) => {
                                        trace!("Read {} bytes from control socket", n);

                                        let tmp = unsafe { assume_init(&tmp[..tmp.len()]) };

                                        if tmp.len() != constants::NET_BUFFER_CAPACITY {
                                            error!("Buffer length differs from BUFFER_CAPACITY! Length: {}", tmp.len());
                                        }

                                        let result = protocol::Request::decode_length_delimited(
                                            &mut Cursor::new(&tmp),
                                        );
                                        match result {
                                            Ok(request) => match request.request_type() {
                                                protocol::RequestType::Status => {
                                                    trace!("Get Status");

                                                    let mut response =
                                                        protocol::Response::default();
                                                    response.set_response_type(
                                                        protocol::RequestType::Status,
                                                    );

                                                    let tmp = "Eruption";
                                                    response.payload = Some(ResponsePayload::Data(
                                                        tmp.as_bytes().to_vec(),
                                                    ));

                                                    let mut buf = Vec::new();
                                                    response.encode_length_delimited(&mut buf)?;

                                                    // send data
                                                    match socket.send(&buf) {
                                                        Ok(_n) => {}

                                                        Err(_e) => {
                                                            return Err(SdkPluginError::PluginError {
                                                                description: "Lost connection to Eruption SDK client".to_owned(),
                                                            }
                                                                .into());
                                                        }
                                                    }
                                                }

                                                protocol::RequestType::SetCanvas => {
                                                    trace!("Set canvas");

                                                    let RequestPayload::Data(payload_map) =
                                                        request.payload.unwrap();

                                                    let mut led_map = [RGBA {
                                                        r: 0,
                                                        g: 0,
                                                        b: 0,
                                                        a: 0,
                                                    };
                                                        constants::CANVAS_SIZE];

                                                    let mut i = 0;
                                                    let mut cntr = 0;

                                                    loop {
                                                        led_map[cntr] = RGBA {
                                                            r: payload_map[i],
                                                            g: payload_map[i + 1],
                                                            b: payload_map[i + 2],
                                                            a: payload_map[i + 3],
                                                        };

                                                        i += 4;
                                                        cntr += 1;

                                                        if cntr >= led_map.len()
                                                            || i >= payload_map.len()
                                                        {
                                                            break;
                                                        }
                                                    }

                                                    LED_MAP.write().copy_from_slice(&led_map);

                                                    SDK_SUPPORT_ACTIVE
                                                        .store(true, Ordering::SeqCst);

                                                    script::FRAME_GENERATION_COUNTER
                                                        .fetch_add(1, Ordering::SeqCst);

                                                    let mut response =
                                                        protocol::Response::default();
                                                    response.set_response_type(
                                                        protocol::RequestType::Noop,
                                                    );

                                                    let mut buf = Vec::new();
                                                    response.encode_length_delimited(&mut buf)?;

                                                    // send data
                                                    match socket.send(&buf) {
                                                        Ok(_n) => {}

                                                        Err(_e) => {
                                                            return Err(SdkPluginError::PluginError {
                                                                description: "Lost connection to Eruption SDK client".to_owned(),
                                                            }
                                                                .into());
                                                        }
                                                    }
                                                }

                                                protocol::RequestType::NotifyHotplug => {
                                                    trace!("Notify hotplug");

                                                    let RequestPayload::Data(payload_hotplug_info) =
                                                        request.payload.unwrap();

                                                    let config = bincode::config::standard();
                                                    let hotplug_info: HotplugInfo =
                                                        bincode::decode_from_slice(
                                                            &payload_hotplug_info,
                                                            config,
                                                        )?
                                                        .0;

                                                    info!("Hotplug event received, trying to claim newly added devices now...");

                                                    claim_hotplugged_devices(&hotplug_info)?;

                                                    // we need to terminate and then re-enter the main loop to update all global state
                                                    crate::REENTER_MAIN_LOOP
                                                        .store(true, Ordering::SeqCst);

                                                    let mut response =
                                                        protocol::Response::default();
                                                    response.set_response_type(
                                                        protocol::RequestType::Noop,
                                                    );

                                                    let mut buf = Vec::new();
                                                    response.encode_length_delimited(&mut buf)?;

                                                    // send data
                                                    match socket.send(&buf) {
                                                        Ok(_n) => {}

                                                        Err(_e) => {
                                                            return Err(SdkPluginError::PluginError {
                                                                description: "Lost connection to Eruption SDK client".to_owned(),
                                                            }
                                                                .into());
                                                        }
                                                    }
                                                }

                                                protocol::RequestType::Noop => {
                                                    /* Do nothing */

                                                    trace!("NOOP");
                                                }
                                            },

                                            Err(e) => {
                                                error!("Protocol error: {}", e);

                                                // break 'EVENT_LOOP;
                                            }
                                        }
                                    }

                                    Err(_e) => {
                                        return Err(SdkPluginError::PluginError {
                                            description: "Lost connection to Eruption SDK client"
                                                .to_owned(),
                                        }
                                        .into());
                                    }
                                }
                            }

                            if SDK_SUPPORT_ACTIVE.load(Ordering::SeqCst) {
                                thread::sleep(Duration::from_millis(1));
                            } else {
                                thread::sleep(Duration::from_millis(15));
                            }
                        }
                    }

                    Err(_e) => {
                        return Err(SdkPluginError::PluginError {
                            description: "Lost connection to Eruption SDK client".to_owned(),
                        }
                        .into());
                    }
                }
            }
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl Plugin for SdkSupportPlugin {
    fn get_name(&self) -> String {
        "SDK Support".to_string()
    }

    fn get_description(&self) -> String {
        "Support for the Eruption SDK".to_string()
    }

    async fn initialize(&mut self) -> plugins::Result<()> {
        Self::initialize_socket()?;
        Self::start_control_thread()?;

        // events::register_observer(|event: &events::Event| {
        //     match event {
        //         events::Event::KeyDown(_index) => {}

        //         events::Event::KeyUp(_index) => {}

        //         _ => (),
        //     };

        //     Ok(true) // event has been processed
        // });

        Ok(())
    }

    fn register_lua_funcs(&self, lua_ctx: &Lua) -> mlua::Result<()> {
        let _globals = lua_ctx.globals();

        // let get_current_slot =
        //     lua_ctx.create_function(move |_, ()| Ok(SdkSupportPlugin::get_current_slot()))?;
        // globals.set("get_current_slot", get_current_slot)?;

        Ok(())
    }

    async fn main_loop_hook(&self, _ticks: u64) {}

    fn sync_main_loop_hook(&self, _ticks: u64) {}

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
