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

use clap::{IntoApp, Parser};
use clap_complete::Shell;
use color_eyre::Help;
use colored::*;
use comfy_table::modifiers::UTF8_ROUND_CORNERS;
use comfy_table::presets::UTF8_FULL;
use comfy_table::{Cell, CellAlignment, ContentArrangement, Table};
use config::Config;
use crossbeam::channel::unbounded;
use dbus::nonblock;
use dbus::nonblock::stdintf::org_freedesktop_dbus::Properties;
use dbus_tokio::connection;
use eyre::Context;
use i18n_embed::{
    fluent::{fluent_language_loader, FluentLanguageLoader},
    DesktopLanguageRequester,
};
use lazy_static::lazy_static;
use manifest::GetAttr;
use parking_lot::Mutex;
use profiles::GetAttr as GetAttrProfile;
use rust_embed::RustEmbed;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use std::{collections::HashMap, path::PathBuf};
use std::{env, thread};
use std::{process, sync::Arc};

mod constants;
mod dbus_client;
mod device;
mod manifest;
mod profiles;
mod util;

type Result<T> = std::result::Result<T, eyre::Error>;

#[derive(RustEmbed)]
#[folder = "i18n"] // path to the compiled localization resources
struct Localizations;

lazy_static! {
    /// Global configuration
    pub static ref STATIC_LOADER: Arc<Mutex<Option<FluentLanguageLoader>>> = Arc::new(Mutex::new(None));
}

#[allow(unused)]
macro_rules! tr {
    ($message_id:literal) => {{
        let loader = $crate::STATIC_LOADER.lock();
        let loader = loader.as_ref().unwrap();

        i18n_embed_fl::fl!(loader, $message_id)
    }};

    ($message_id:literal, $($args:expr),*) => {{
        let loader = $crate::STATIC_LOADER.lock();
        let loader = loader.as_ref().unwrap();

        i18n_embed_fl::fl!(loader, $message_id, $($args), *)
    }};
}

lazy_static! {
    /// Global configuration
    pub static ref CONFIG: Arc<Mutex<Option<config::Config>>> = Arc::new(Mutex::new(None));

    /// Global "quit" status flag
    pub static ref QUIT: AtomicBool = AtomicBool::new(false);
}

#[derive(Debug, thiserror::Error)]
pub enum MainError {
    #[error("Unknown error: {description}")]
    UnknownError { description: String },
}

lazy_static! {
    static ref ABOUT: String = tr!("about");
    static ref VERBOSE_ABOUT: String = tr!("verbose-about");
    static ref COMPLETIONS_ABOUT: String = tr!("completions-about");
    static ref CONFIG_ABOUT: String = tr!("config-about");
    static ref DEVICES_ABOUT: String = tr!("devices-about");
    static ref STATUS_ABOUT: String = tr!("status-about");
    static ref SWITCH_ABOUT: String = tr!("switch-about");
    static ref PROFILES_ABOUT: String = tr!("profiles-about");
    static ref NAMES_ABOUT: String = tr!("names-about");
    static ref SCRIPTS_ABOUT: String = tr!("scripts-about");
    static ref PARAM_ABOUT: String = tr!("param-about");
}

/// Supported command line arguments
#[derive(Debug, clap::Parser)]
#[clap(
    version = env!("CARGO_PKG_VERSION"),
    author = "X3n0m0rph59 <x3n0m0rph59@gmail.com>",
    about = ABOUT.as_str(),
)]
pub struct Options {
    /// Verbose mode (-v, -vv, -vvv, etc.)
    #[clap(help(VERBOSE_ABOUT.as_str()), short, long, parse(from_occurrences))]
    verbose: u8,

    /// Repeat output until ctrl+c is pressed
    #[clap(short, long)]
    repeat: bool,

    /// Sets the configuration file to use
    #[clap(short = 'c', long)]
    config: Option<String>,

    #[clap(subcommand)]
    command: Subcommands,
}

// Sub-commands
#[derive(Debug, clap::Parser)]
pub enum Subcommands {
    #[clap(about(CONFIG_ABOUT.as_str()))]
    Config {
        #[clap(subcommand)]
        command: ConfigSubcommands,
    },

    #[clap(about(DEVICES_ABOUT.as_str()))]
    Devices {
        #[clap(subcommand)]
        command: DevicesSubcommands,
    },

    #[clap(about(STATUS_ABOUT.as_str()))]
    Status {
        #[clap(subcommand)]
        command: StatusSubcommands,
    },

    #[clap(about(SWITCH_ABOUT.as_str()))]
    Switch {
        #[clap(subcommand)]
        command: SwitchSubcommands,
    },

    #[clap(about(PROFILES_ABOUT.as_str()))]
    Profiles {
        #[clap(subcommand)]
        command: ProfilesSubcommands,
    },

    #[clap(about(NAMES_ABOUT.as_str()))]
    Names {
        #[clap(subcommand)]
        command: NamesSubcommands,
    },

    #[clap(about(SCRIPTS_ABOUT.as_str()))]
    Scripts {
        #[clap(subcommand)]
        command: ScriptsSubcommands,
    },

    #[clap(about(PARAM_ABOUT.as_str()))]
    Param {
        script: Option<String>,
        parameter: Option<String>,
        value: Option<String>,
    },

    #[clap(about(COMPLETIONS_ABOUT.as_str()))]
    Completions {
        // #[clap(subcommand)]
        shell: Shell,
    },
}

/// Sub-commands of the "config" command
#[derive(Debug, clap::Parser)]
pub enum ConfigSubcommands {
    /// Get or set the global brightness of the LEDs
    Brightness { brightness: Option<i64> },

    /// Get or set the state of SoundFX
    Soundfx { enable: Option<bool> },
}

/// Sub-commands of the "devices" command
#[derive(Debug, clap::Parser)]
pub enum DevicesSubcommands {
    /// List connected devices and their indices (run this first)
    #[clap(display_order = 0)]
    List,

    /// Get information about a specific device
    #[clap(display_order = 1)]
    Info { device: String },

    /// Get status of a specific device
    #[clap(display_order = 2)]
    Status { device: String },

    /// Get or set the device specific brightness of the LEDs
    // #[clap(display_order = 3)]
    Brightness {
        device: String,
        brightness: Option<i64>,
    },

    /// Get or set the current profile (applicable for some devices)
    // #[clap(display_order = 4)]
    Profile {
        device: String,
        profile: Option<i32>,
    },

    /// Get or set the DPI parameter (applicable for some mice)
    // #[clap(display_order = 5)]
    Dpi { device: String, dpi: Option<i32> },

    /// Get or set the bus poll rate
    // #[clap(display_order = 6)]
    Rate { device: String, rate: Option<i32> },

    /// Get or set the debounce parameter (applicable for some mice)
    // #[clap(display_order = 7)]
    Debounce {
        device: String,
        enable: Option<bool>,
    },

    /// Get or set the DCU parameter (applicable for some mice)
    // #[clap(display_order = 8)]
    Distance { device: String, param: Option<i32> },

    /// Get or set the angle-snapping parameter (applicable for some mice)
    // #[clap(display_order = 9)]
    AngleSnapping {
        device: String,
        enable: Option<bool>,
    },
}

/// Sub-commands of the "status" command
#[derive(Debug, clap::Parser)]
pub enum StatusSubcommands {
    /// Shows the currently active profile
    Profile,

    /// Shows the currently active slot
    Slot,
}

/// Sub-commands of the "switch" command
#[derive(Debug, clap::Parser)]
pub enum SwitchSubcommands {
    /// Switch profiles
    Profile { profile_name: String },

    /// Switch slots
    Slot { index: usize },
}

/// Sub-commands of the "profiles" command
#[derive(Debug, clap::Parser)]
pub enum ProfilesSubcommands {
    /// Show info about a profile
    Info { profile_name: String },

    /// Edit a profile
    Edit { profile_name: String },

    /// List available profiles
    List,
}

/// Subcommands of the "names" command
#[derive(Debug, clap::Parser)]
pub enum NamesSubcommands {
    /// List slot names
    List,

    /// Set the name of a single profile slot
    Set { slot_index: usize, name: String },

    /// Set all the profile slot names at once
    SetAll { names: Vec<String> },
}

/// Subcommands of the "scripts" command
#[derive(Debug, clap::Parser)]
pub enum ScriptsSubcommands {
    /// Show info about a script
    Info { script_name: String },

    /// Edit a script
    Edit { script_name: String },

    /// List available scripts
    List,
}

/// Print license information
#[allow(dead_code)]
fn print_header() {
    println!("{}", tr!("license-header"));
    println!();
}

/// Returns a connection to the D-Bus system bus using the specified `path`
pub async fn dbus_system_bus(
    path: &str,
) -> Result<dbus::nonblock::Proxy<'_, Arc<dbus::nonblock::SyncConnection>>> {
    let (resource, conn) = connection::new_system_sync()?;

    tokio::spawn(async {
        let err = resource.await;
        panic!("Lost connection to D-Bus: {}", err);
    });

    let proxy = nonblock::Proxy::new(
        "org.eruption",
        path,
        Duration::from_secs(constants::DBUS_TIMEOUT_MILLIS as u64),
        conn,
    );

    Ok(proxy)
}

/// Switch the currently active profile
pub async fn switch_profile(name: &str) -> Result<()> {
    let file_name = name.to_owned();

    let (_result,): (bool,) = dbus_system_bus("/org/eruption/profile")
        .await?
        .method_call("org.eruption.Profile", "SwitchProfile", (file_name,))
        .await?;

    Ok(())
}

/// Get the index of the currently active slot
pub async fn get_active_slot() -> Result<usize> {
    let result: u64 = dbus_system_bus("/org/eruption/slot")
        .await?
        .get("org.eruption.Slot", "ActiveSlot")
        .await?;

    Ok(result as usize)
}

/// Get the name of the currently active profile
pub async fn get_active_profile() -> Result<String> {
    let result: String = dbus_system_bus("/org/eruption/profile")
        .await?
        .get("org.eruption.Profile", "ActiveProfile")
        .await?;

    Ok(result)
}

/// Switch the currently active slot
pub async fn switch_slot(index: usize) -> Result<()> {
    let (_result,): (bool,) = dbus_system_bus("/org/eruption/slot")
        .await?
        .method_call("org.eruption.Slot", "SwitchSlot", (index as u64,))
        .await?;

    Ok(())
}

/// Get the names of the profile slots
pub async fn get_slot_names() -> Result<Vec<String>> {
    let result: Vec<String> = dbus_system_bus("/org/eruption/slot")
        .await?
        .get("org.eruption.Slot", "SlotNames")
        .await?;

    Ok(result)
}

/// Set the names of the profile slots
pub async fn set_slot_names(names: &[String]) -> Result<()> {
    let arg = Box::new(names);

    let _result = dbus_system_bus("/org/eruption/slot")
        .await?
        .set("org.eruption.Slot", "SlotNames", arg)
        .await?;

    Ok(())
}

/// Set the name of a single profile slot
pub async fn set_slot_name(slot_index: usize, name: String) -> Result<()> {
    let mut result = get_slot_names().await?;

    result[slot_index] = name;
    set_slot_names(&result).await?;

    Ok(())
}

/// Enumerate all available profiles
pub async fn get_profiles() -> Result<Vec<(String, String)>> {
    let (result,): (Vec<(String, String)>,) = dbus_system_bus("/org/eruption/profile")
        .await?
        .method_call("org.eruption.Profile", "EnumProfiles", ())
        .await?;

    Ok(result)
}

/// Enumerate all available devices
pub async fn get_devices() -> Result<(Vec<(u16, u16)>, Vec<(u16, u16)>, Vec<(u16, u16)>)> {
    let ((keyboards, mice, misc),): ((Vec<(u16, u16)>, Vec<(u16, u16)>, Vec<(u16, u16)>),) =
        dbus_system_bus("/org/eruption/devices")
            .await?
            .method_call("org.eruption.Device", "GetManagedDevices", ())
            .await?;

    Ok((keyboards, mice, misc))
}

/// Get device specific status
pub async fn get_device_status(device: u64) -> Result<HashMap<String, String>> {
    let (status,): (String,) = dbus_system_bus("/org/eruption/devices")
        .await?
        .method_call("org.eruption.Device", "GetDeviceStatus", (device as u64,))
        .await?;

    let result: HashMap<String, String> = serde_json::from_str(&status)?;

    Ok(result)
}

/// Get a device specific config param
pub async fn get_device_config(device: u64, param: &str) -> Result<String> {
    let (result,): (String,) = dbus_system_bus("/org/eruption/devices")
        .await?
        .method_call(
            "org.eruption.Device",
            "GetDeviceConfig",
            (device, param.to_owned()),
        )
        .await?;

    Ok(result)
}

/// Set a device specific config param
pub async fn set_device_config(device: u64, param: &str, value: &str) -> Result<()> {
    let (_result,): (bool,) = dbus_system_bus("/org/eruption/devices")
        .await?
        .method_call(
            "org.eruption.Device",
            "SetDeviceConfig",
            (device, param.to_owned(), value.to_owned()),
        )
        .await?;

    Ok(())
}

/// Enumerate all available scripts
pub fn get_script_list() -> Result<Vec<(String, String)>> {
    let scripts = util::enumerate_scripts()?;

    let result = scripts
        .iter()
        .map(|s| {
            (
                format!("{} - {}", s.name.clone(), s.description.clone()),
                s.script_file.to_string_lossy().to_string(),
            )
        })
        .collect();

    Ok(result)
}

// global configuration options

/// Get the current brightness value
pub async fn get_brightness() -> Result<i64> {
    let result = dbus_system_bus("/org/eruption/config")
        .await?
        .get("org.eruption.Config", "Brightness")
        .await?;

    Ok(result)
}

/// Set the current brightness value
pub async fn set_brightness(brightness: i64) -> Result<()> {
    let arg = Box::new(brightness as i64);

    let _result = dbus_system_bus("/org/eruption/config")
        .await?
        .set("org.eruption.Config", "Brightness", arg)
        .await?;

    Ok(())
}

/// Returns true when SoundFX is enabled
pub async fn get_sound_fx() -> Result<bool> {
    let result = dbus_system_bus("/org/eruption/config")
        .await?
        .get("org.eruption.Config", "EnableSfx")
        .await?;

    Ok(result)
}

/// Set SoundFX state to `enabled`
pub async fn set_sound_fx(enabled: bool) -> Result<()> {
    let arg = Box::new(enabled);

    let _result = dbus_system_bus("/org/eruption/config")
        .await?
        .set("org.eruption.Config", "EnableSfx", arg)
        .await?;

    Ok(())
}

async fn print_device_header(device: u64) -> Result<()> {
    let mut base_index = 0;

    let (keyboards, mice, misc) = get_devices().await?;

    print!("Selected device: ");

    if !keyboards.is_empty() {
        for (_index, dev) in keyboards.iter().enumerate() {
            if base_index == device {
                println!(
                    // "{}: ID: {}:{} {} {}",
                    // format!("{:02}", base_index).bold(),
                    // format!("{:04x}", dev.0),
                    // format!("{:04x}", dev.1),
                    "{} {}",
                    device::get_device_make(dev.0, dev.1)
                        .unwrap_or("<unknown make>")
                        .bold(),
                    device::get_device_model(dev.0, dev.1)
                        .unwrap_or("<unknown model>")
                        .bold(),
                );
            }

            base_index += 1;
        }
    }

    if !mice.is_empty() {
        for (_index, dev) in mice.iter().enumerate() {
            if base_index == device {
                println!(
                    // "{}: ID: {}:{} {} {}",
                    // format!("{:02}", base_index).bold(),
                    // format!("{:04x}", dev.0),
                    // format!("{:04x}", dev.1),
                    "{} {}",
                    device::get_device_make(dev.0, dev.1)
                        .unwrap_or("<unknown make>")
                        .bold(),
                    device::get_device_model(dev.0, dev.1)
                        .unwrap_or("<unknown model>")
                        .bold(),
                );
            }

            base_index += 1;
        }
    }

    if !misc.is_empty() {
        for (_index, dev) in misc.iter().enumerate() {
            if base_index == device {
                println!(
                    // "{}: ID: {}:{} {} {}",
                    // format!("{:02}", base_index).bold(),
                    // format!("{:04x}", dev.0),
                    // format!("{:04x}", dev.1),
                    "{} {}",
                    device::get_device_make(dev.0, dev.1)
                        .unwrap_or("<unknown make>")
                        .bold(),
                    device::get_device_model(dev.0, dev.1)
                        .unwrap_or("<unknown model>")
                        .bold(),
                );
            }

            base_index += 1;
        }
    }

    Ok(())
}

pub async fn async_main() -> std::result::Result<(), eyre::Error> {
    cfg_if::cfg_if! {
        if #[cfg(debug_assertions)] {
            color_eyre::config::HookBuilder::default()
            .panic_section("Please consider reporting a bug at https://github.com/X3n0m0rph59/eruption")
            .install()?;
        } else {
            color_eyre::config::HookBuilder::default()
            .panic_section("Please consider reporting a bug at https://github.com/X3n0m0rph59/eruption")
            .display_env_section(false)
            .install()?;
        }
    }

    // print a license header, except if we are generating shell completions
    if !env::args().any(|a| a.eq_ignore_ascii_case("completions")) && env::args().count() < 2 {
        print_header();
    }

    // register ctrl-c handler
    let (ctrl_c_tx, _ctrl_c_rx) = unbounded();
    ctrlc::set_handler(move || {
        QUIT.store(true, Ordering::SeqCst);

        ctrl_c_tx.send(true).unwrap_or_else(|e| {
            log::error!(
                "{}",
                tr!("could-not-send-on-channel", message = e.to_string())
            );
        });
    })
    .unwrap_or_else(|e| {
        log::error!(
            "{}",
            tr!("could-not-set-ctrl-c-handler", message = e.to_string())
        )
    });

    let opts = Options::parse();

    // process configuration file
    let config_file = opts
        .config
        .unwrap_or(constants::DEFAULT_CONFIG_FILE.to_string());

    let config = Config::builder()
        .add_source(config::File::new(&config_file, config::FileFormat::Toml))
        .build()
        .unwrap_or_else(|e| {
            log::error!("{}", tr!("could-not-parse-config", message = e.to_string()));
            process::exit(4);
        });

    *CONFIG.lock() = Some(config);

    match opts.command {
        // configuration related sub-commands
        Subcommands::Config { command } => match command {
            ConfigSubcommands::Brightness { brightness } => {
                if let Some(brightness) = brightness {
                    set_brightness(brightness)
                        .await
                        .wrap_err("Could not connect to the Eruption daemon")
                        .suggestion("Please verify that the Eruption daemon is running")?;
                } else {
                    let result = get_brightness()
                        .await
                        .wrap_err("Could not connect to the Eruption daemon")
                        .suggestion("Please verify that the Eruption daemon is running")?;
                    println!(
                        "{}",
                        format!("Global brightness: {}", format!("{}%", result).bold())
                    );
                }
            }

            ConfigSubcommands::Soundfx { enable } => {
                if let Some(enable) = enable {
                    set_sound_fx(enable)
                        .await
                        .wrap_err("Could not connect to the Eruption daemon")
                        .suggestion("Please verify that the Eruption daemon is running")?;
                } else {
                    let result = get_sound_fx()
                        .await
                        .wrap_err("Could not connect to the Eruption daemon")
                        .suggestion("Please verify that the Eruption daemon is running")?;
                    println!(
                        "{}",
                        format!("SoundFX enabled: {}", format!("{}", result).bold())
                    );
                }
            }
        },

        // device specific sub-commands
        Subcommands::Devices { command } => match command {
            DevicesSubcommands::List => {
                let mut base_index = 0;

                let (keyboards, mice, misc) = get_devices()
                    .await
                    .wrap_err("Could not connect to the Eruption daemon")
                    .suggestion("Please verify that the Eruption daemon is running")?;

                if opts.verbose > 0 {
                    println!(
                        "{}",
                        r#"
 Use the `eruptionctl devices list` sub-command to find out the index of the device that
 you want to operate on. All the other device-related commands require a device index.

 Examples:

 Set the brightness of the first connected keyboard to 80 percent:

    $ eruptionctl devices brightness 0 80


 Query the DPI configuration of the first connected mouse (second device):

    $ eruptionctl devices dpi 1

"#
                    );
                }

                println!("{}\n", tr!("dumping-devices").bold());

                println!("{}", tr!("keyboard-devices"));

                if keyboards.is_empty() {
                    println!("{}", "<No supported devices detected>\n".italic());
                } else {
                    for (_index, dev) in keyboards.iter().enumerate() {
                        if opts.verbose > 0 {
                            println!(
                                "Index: {}: ID: {}:{} {} {}",
                                format!("{:02}", base_index).bold(),
                                format!("{:04x}", dev.0),
                                format!("{:04x}", dev.1),
                                device::get_device_make(dev.0, dev.1)
                                    .unwrap_or("<unknown make>")
                                    .bold(),
                                device::get_device_model(dev.0, dev.1)
                                    .unwrap_or("<unknown model>")
                                    .bold()
                            );
                        } else {
                            println!(
                                "{}: {} {}",
                                format!("{:02}", base_index).bold(),
                                device::get_device_make(dev.0, dev.1)
                                    .unwrap_or("<unknown make>")
                                    .bold(),
                                device::get_device_model(dev.0, dev.1)
                                    .unwrap_or("<unknown model>")
                                    .bold()
                            );
                        }

                        base_index += 1;
                    }
                }

                println!("\n{}", tr!("mouse-devices"));

                if mice.is_empty() {
                    println!("{}", "<No supported devices detected>\n".italic());
                } else {
                    for (_index, dev) in mice.iter().enumerate() {
                        if opts.verbose > 0 {
                            println!(
                                "Index: {}: ID: {}:{} {} {}",
                                format!("{:02}", base_index).bold(),
                                format!("{:04x}", dev.0),
                                format!("{:04x}", dev.1),
                                device::get_device_make(dev.0, dev.1)
                                    .unwrap_or("<unknown make>")
                                    .bold(),
                                device::get_device_model(dev.0, dev.1)
                                    .unwrap_or("<unknown model>")
                                    .bold()
                            );
                        } else {
                            println!(
                                "{}: {} {}",
                                format!("{:02}", base_index).bold(),
                                device::get_device_make(dev.0, dev.1)
                                    .unwrap_or("<unknown make>")
                                    .bold(),
                                device::get_device_model(dev.0, dev.1)
                                    .unwrap_or("<unknown model>")
                                    .bold()
                            );
                        }

                        base_index += 1;
                    }
                }

                println!("\n{}", tr!("misc-devices"));

                if misc.is_empty() {
                    println!("{}", "<No supported devices detected>\n".italic());
                } else {
                    for (_index, dev) in misc.iter().enumerate() {
                        if opts.verbose > 0 {
                            println!(
                                "Index: {}: ID: {}:{} {} {}",
                                format!("{:02}", base_index).bold(),
                                format!("{:04x}", dev.0),
                                format!("{:04x}", dev.1),
                                device::get_device_make(dev.0, dev.1)
                                    .unwrap_or("<unknown make>")
                                    .bold(),
                                device::get_device_model(dev.0, dev.1)
                                    .unwrap_or("<unknown model>")
                                    .bold()
                            );
                        } else {
                            println!(
                                "{}: {} {}",
                                format!("{:02}", base_index).bold(),
                                device::get_device_make(dev.0, dev.1)
                                    .unwrap_or("<unknown make>")
                                    .bold(),
                                device::get_device_model(dev.0, dev.1)
                                    .unwrap_or("<unknown model>")
                                    .bold()
                            );
                        }

                        base_index += 1;
                    }
                }
            }

            DevicesSubcommands::Info { device } => {
                let device = device.parse::<u64>()?;

                print_device_header(device)
                    .await
                    .wrap_err("Could not connect to the Eruption daemon")
                    .suggestion("Please verify that the Eruption daemon is running")?;

                let result = get_device_config(device, "info").await?;

                println!("{}", format!("{}", result.bold()));
            }

            DevicesSubcommands::Status { device } => {
                let device = device.parse::<u64>()?;

                print_device_header(device)
                    .await
                    .wrap_err("Could not connect to the Eruption daemon")
                    .suggestion("Please verify that the Eruption daemon is running")?;

                let term = console::Term::stdout();

                // stores how many lines we printed in the previous iteration
                let mut prev = 0;

                loop {
                    let result = get_device_status(device).await?;

                    let mut table = Table::new();
                    table
                        .load_preset(UTF8_FULL)
                        .apply_modifier(UTF8_ROUND_CORNERS)
                        .set_content_arrangement(ContentArrangement::Dynamic)
                        .set_table_width(40)
                        .set_header(vec!["Parameter", "Value"]);

                    // counts the number of lines that we printed
                    let mut cntr = 3;

                    let mut v = result.iter().collect::<Vec<(&String, &String)>>();
                    v.sort_by_key(|&v| v.0);

                    v.iter().for_each(|(k, v)| {
                        table.add_row(vec![
                            Cell::new(k.to_owned()).set_alignment(CellAlignment::Left),
                            Cell::new(v.to_owned()).set_alignment(CellAlignment::Right),
                        ]);

                        cntr += 2;
                    });

                    // empty table requires special handling
                    if cntr <= 3 {
                        cntr = 4
                    }

                    term.clear_last_lines(prev)?;
                    prev = cntr;

                    println!("{}", table);

                    if !opts.repeat || QUIT.load(Ordering::SeqCst) {
                        break;
                    }

                    thread::sleep(Duration::from_millis(250));
                }
            }

            DevicesSubcommands::Profile { device, profile } => {
                let device = device.parse::<u64>()?;

                print_device_header(device)
                    .await
                    .wrap_err("Could not connect to the Eruption daemon")
                    .suggestion("Please verify that the Eruption daemon is running")?;

                if let Some(profile) = profile {
                    let value = &format!("{}", profile);

                    set_device_config(device, "profile", value).await?;
                } else {
                    let result = get_device_config(device, "profile").await?;

                    println!("{}", format!("Current profile: {}", result.bold()));
                }
            }

            DevicesSubcommands::Dpi { device, dpi } => {
                let device = device.parse::<u64>()?;

                print_device_header(device)
                    .await
                    .wrap_err("Could not connect to the Eruption daemon")
                    .suggestion("Please verify that the Eruption daemon is running")?;

                if let Some(dpi) = dpi {
                    let value = &format!("{}", dpi);

                    set_device_config(device, "dpi", value).await?
                } else {
                    let result = get_device_config(device, "dpi").await?;

                    println!("{}", format!("DPI config: {}", result.bold()));
                }
            }

            DevicesSubcommands::Rate { device, rate } => {
                let device = device.parse::<u64>()?;

                print_device_header(device)
                    .await
                    .wrap_err("Could not connect to the Eruption daemon")
                    .suggestion("Please verify that the Eruption daemon is running")?;

                if let Some(rate) = rate {
                    let value = &format!("{}", rate);

                    set_device_config(device, "rate", value).await?
                } else {
                    let result = get_device_config(device, "rate").await?;

                    println!("{}", format!("Poll rate: {}", result.bold()));
                }
            }

            DevicesSubcommands::Distance { device, param } => {
                let device = device.parse::<u64>()?;

                print_device_header(device)
                    .await
                    .wrap_err("Could not connect to the Eruption daemon")
                    .suggestion("Please verify that the Eruption daemon is running")?;

                if let Some(param) = param {
                    let value = &format!("{}", param);

                    set_device_config(device, "dcu", value).await?
                } else {
                    let result = get_device_config(device, "dcu").await?;

                    println!("{}", format!("DCU config: {}", result.bold()));
                }
            }

            DevicesSubcommands::AngleSnapping { device, enable } => {
                let device = device.parse::<u64>()?;

                print_device_header(device)
                    .await
                    .wrap_err("Could not connect to the Eruption daemon")
                    .suggestion("Please verify that the Eruption daemon is running")?;

                if let Some(enable) = enable {
                    let value = &format!("{}", enable);

                    set_device_config(device, "angle-snapping", value).await?
                } else {
                    let result = get_device_config(device, "angle-snapping").await?;

                    println!("{}", format!("Angle-snapping: {}", result.bold()));
                }
            }

            DevicesSubcommands::Debounce { device, enable } => {
                let device = device.parse::<u64>()?;

                print_device_header(device)
                    .await
                    .wrap_err("Could not connect to the Eruption daemon")
                    .suggestion("Please verify that the Eruption daemon is running")?;

                if let Some(enable) = enable {
                    let value = &format!("{}", enable);

                    set_device_config(device, "debounce", value).await?
                } else {
                    let result = get_device_config(device, "debounce").await?;

                    println!("{}", format!("Debounce: {}", result.bold()));
                }
            }

            DevicesSubcommands::Brightness { device, brightness } => {
                let device = device.parse::<u64>()?;

                print_device_header(device)
                    .await
                    .wrap_err("Could not connect to the Eruption daemon")
                    .suggestion("Please verify that the Eruption daemon is running")?;

                if let Some(brightness) = brightness {
                    let value = &format!("{}", brightness);

                    set_device_config(device, "brightness", value).await?
                } else {
                    let result = get_device_config(device, "brightness").await?;

                    println!("{}", format!("Device brightness: {}%", result.bold()));
                }
            }
        },

        // profile related sub-commands
        Subcommands::Profiles { command } => match command {
            ProfilesSubcommands::Edit { profile_name } => {
                let profiles = util::enumerate_profiles().unwrap_or_else(|_| vec![]);

                if let Some(profile) = profiles
                    .iter()
                    .find(|p| *p.profile_file.to_string_lossy() == profile_name)
                {
                    util::edit_file(&profile.profile_file)?
                } else {
                    eprintln!("No matches found");
                }
            }

            ProfilesSubcommands::List => {
                for p in get_profiles()
                    .await
                    .wrap_err("Could not connect to the Eruption daemon")
                    .suggestion("Please verify that the Eruption daemon is running")?
                {
                    println!("{}: {}", p.0.bold(), p.1);
                }
            }

            ProfilesSubcommands::Info { profile_name } => {
                let profiles = util::enumerate_profiles().unwrap_or_else(|_| vec![]);

                let empty = HashMap::new();

                if let Some(profile) = profiles
                    .iter()
                    .find(|p| *p.profile_file.to_string_lossy() == profile_name)
                {
                    println!(
                        "Profile:\t{} ({})\nDescription:\t{}\nScripts:\t{:?}\n\n{:#?}",
                        profile.name,
                        profile.id,
                        profile.description,
                        profile.active_scripts,
                        profile.config.as_ref().unwrap_or(&empty),
                    );
                } else {
                    eprintln!("No matches found");
                }
            }
        },

        // naming related sub-commands
        Subcommands::Names { command } => match command {
            NamesSubcommands::List => {
                let slot_names = get_slot_names()
                    .await
                    .wrap_err("Could not connect to the Eruption daemon")
                    .suggestion("Please verify that the Eruption daemon is running")?;

                for (index, name) in slot_names.iter().enumerate() {
                    let s = format!("{}", index + 1);
                    println!("{}: {}", s.bold(), name);
                }
            }

            NamesSubcommands::Set { slot_index, name } => {
                if slot_index > 0 && slot_index <= constants::NUM_SLOTS {
                    set_slot_name(slot_index - 1, name)
                        .await
                        .wrap_err("Could not connect to the Eruption daemon")
                        .suggestion("Please verify that the Eruption daemon is running")?;
                } else {
                    eprintln!("Slot index out of bounds");
                }
            }

            NamesSubcommands::SetAll { names } => {
                if names.len() == constants::NUM_SLOTS {
                    set_slot_names(&names)
                        .await
                        .wrap_err("Could not connect to the Eruption daemon")
                        .suggestion("Please verify that the Eruption daemon is running")?;
                } else {
                    eprintln!("Elements do not match number of slots");
                }
            }
        },

        // script related sub-commands
        Subcommands::Scripts { command } => match command {
            ScriptsSubcommands::Edit { script_name } => {
                let scripts = util::enumerate_scripts()?;

                if let Some(script) = scripts
                    .iter()
                    .find(|s| *s.script_file.to_string_lossy() == script_name)
                {
                    util::edit_file(&script.script_file)?
                } else {
                    eprintln!("No matches found");
                }
            }

            ScriptsSubcommands::List => {
                for s in get_script_list()? {
                    println!("{}: {}", s.0.bold(), s.1);
                }
            }

            ScriptsSubcommands::Info { script_name } => {
                let scripts = util::enumerate_scripts()?;

                let empty = vec![];

                if let Some(script) = scripts
                    .iter()
                    .find(|s| *s.script_file.to_string_lossy() == script_name)
                {
                    println!(
                        "Lua script:\t{} ({})\nDaemon version:\t{}\nAuthor:\t\t{}\nDescription:\t{}\nTags:\t\t{:?}",
                        script.name,
                        script.version,
                        script.min_supported_version,
                        script.author,
                        script.description,
                        script.tags.as_ref().unwrap_or(&empty),
                    );
                } else {
                    eprintln!("No matches found");
                }
            }
        },

        // parameter
        Subcommands::Param {
            script,
            parameter,
            value,
        } => {
            // determine mode of operation
            if script.is_none() && parameter.is_none() && value.is_none() {
                // list parameters from all scripts in the currently active profile
                let profile_name = get_active_profile().await.map_err(|e| {
                    eprintln!("Could not determine the currently active profile! Is the Eruption daemon running?");
                    e
                })?;

                let profiles = util::enumerate_profiles().unwrap_or_else(|_| vec![]);

                if let Some(profile) = profiles
                    .iter()
                    .find(|&p| *p.profile_file.to_string_lossy() == profile_name)
                {
                    println!(
                        "Profile:\t{} ({})\nDescription:\t{}\nScripts:\t{:?}\n",
                        profile.name, profile.id, profile.description, profile.active_scripts,
                    );

                    // dump parameters set in .profile file

                    println!("Profile parameters:\n");

                    let empty = HashMap::new();

                    let scripts = util::enumerate_scripts()?;

                    for script in &scripts {
                        if profile.active_scripts.contains(&PathBuf::from(
                            script.script_file.file_name().unwrap_or_default(),
                        )) {
                            let config = profile.config.as_ref().unwrap_or(&empty);
                            let config_params = config.get(&script.name);

                            if let Some(config_params) = config_params {
                                for config in config_params.iter() {
                                    // read param value
                                    let value = if config.get_value() == config.get_default() {
                                        (&config.get_value()).to_string().normal()
                                    } else {
                                        (&config.get_value()).to_string().bold()
                                    };

                                    println!(
                                        "\"{}\" {} {} (default: {})",
                                        &script.name,
                                        &config.get_name(),
                                        &value,
                                        &config.get_default(),
                                    );
                                }
                            }
                        }
                    }

                    if opts.verbose > 0 {
                        // dump all available parameters that could be set in the .profile file
                        println!();
                        println!("Available parameters:\n");

                        for script in &scripts {
                            if profile.active_scripts.contains(&PathBuf::from(
                                script.script_file.file_name().unwrap_or_default(),
                            )) {
                                if let Some(config_params) = script.config.as_ref() {
                                    for config in config_params.iter() {
                                        // read param defaults
                                        let value = config.get_default();

                                        println!(
                                            "\"{}\" {} (default: {})",
                                            &script.name,
                                            &config.get_name(),
                                            &value,
                                        );
                                    }
                                }

                                println!();
                            }
                        }
                    }
                } else {
                    eprintln!("Could not load the current profile");
                }
            } else if let Some(script) = script {
                let profile_name = get_active_profile().await.map_err(|e| {
                    eprintln!("Could not determine the currently active profile! Is the Eruption daemon running?");
                    e
                })?;

                if let Some(value) = value {
                    // set a parameter from the specified script in the currently active profile

                    let parameter = parameter.unwrap();

                    let profiles = util::enumerate_profiles().unwrap_or_else(|_| vec![]);

                    if let Some(profile) = profiles
                        .iter()
                        .find(|&p| *p.profile_file.to_string_lossy() == profile_name)
                    {
                        println!(
                            "Profile:\t{} ({})\nDescription:\t{}\nScripts:\t{:?}\n",
                            profile.name, profile.id, profile.description, profile.active_scripts,
                        );

                        let scripts = util::enumerate_scripts()?;

                        for scr in scripts {
                            if scr.name == script {
                                // set param value
                                dbus_client::set_parameter(
                                    &*profile.profile_file.to_string_lossy(),
                                    &*scr.script_file.to_string_lossy(),
                                    &parameter,
                                    &value,
                                )?;

                                println!("\"{}\" {} {}", &scr.name, &parameter, &value.bold(),);

                                break;
                            }
                        }
                    } else {
                        eprintln!("Could not load the current profile");
                    }
                } else if let Some(parameter) = parameter {
                    // list parameters from the specified script in the currently active profile

                    let profiles = util::enumerate_profiles().unwrap_or_else(|_| vec![]);

                    if let Some(profile) = profiles
                        .iter()
                        .find(|&p| *p.profile_file.to_string_lossy() == profile_name)
                    {
                        let empty = HashMap::new();

                        let scripts = util::enumerate_scripts()?;

                        'OUTER_LOOP: for script in scripts {
                            if profile.active_scripts.contains(&PathBuf::from(
                                script.script_file.file_name().unwrap_or_default(),
                            )) {
                                let config = profile.config.as_ref().unwrap_or(&empty);
                                if let Some(config) = config.get(&script.name) {
                                    for config in config.iter() {
                                        if config.get_name() == &parameter {
                                            if let Some(value) = &value {
                                                println!(
                                                    "Profile:\t{} ({})\nDescription:\t{}\nScripts:\t{:?}\n",
                                                    profile.name, profile.id, profile.description, profile.active_scripts,
                                                );

                                                // set param value
                                                dbus_client::set_parameter(
                                                    &*profile.profile_file.to_string_lossy(),
                                                    &*script.script_file.to_string_lossy(),
                                                    &parameter,
                                                    value,
                                                )?;

                                                println!(
                                                    "\"{}\" {} {}",
                                                    &script.name,
                                                    &parameter,
                                                    &value.bold(),
                                                );

                                                break 'OUTER_LOOP;
                                            } else {
                                                println!(
                                                    "Profile:\t{} ({})\nDescription:\t{}\nScripts:\t{:?}\n",
                                                    profile.name, profile.id, profile.description, profile.active_scripts,
                                                );

                                                // read param value
                                                println!(
                                                    "\"{}\" {} {}",
                                                    &script.name,
                                                    &config.get_name(),
                                                    &config.get_value().bold(),
                                                );

                                                break 'OUTER_LOOP;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    } else {
                        eprintln!("No matches found");
                    }
                } else {
                    // list parameters from the specified script
                    println!("Dumping all parameters from the specified script:\n");

                    let scripts = util::enumerate_scripts()?;

                    for scr in scripts {
                        if scr.name == script {
                            for param in scr.config.unwrap_or_else(|| Vec::new()) {
                                println!(
                                    "\"{}\" {} default: {}",
                                    scr.name,
                                    param.get_name().bold(),
                                    param.get_default()
                                );
                            }
                        }
                    }
                }
            } else {
                println!("Nothing to do");
            }
        }

        // convenience operations: switch profile or slot
        Subcommands::Status { command } => match command {
            StatusSubcommands::Profile => {
                let profile_name = get_active_profile()
                    .await
                    .wrap_err("Could not connect to the Eruption daemon")
                    .suggestion("Please verify that the Eruption daemon is running")?;
                println!("Current profile: {}", profile_name.bold());
            }

            StatusSubcommands::Slot => {
                let index = get_active_slot()
                    .await
                    .wrap_err("Could not connect to the Eruption daemon")
                    .suggestion("Please verify that the Eruption daemon is running")?
                    + 1;
                println!("Current slot: {}", format!("{}", index).bold());
            }
        },

        // convenience operations: switch profile or slot
        Subcommands::Switch { command } => match command {
            SwitchSubcommands::Profile { profile_name } => {
                let profile_path = PathBuf::from(&profile_name);

                let profile_name = if profile_path.is_file() {
                    Ok(profile_path)
                } else {
                    util::match_profile_path(&profile_name)
                };

                match profile_name {
                    Ok(profile_name) => {
                        println!(
                            "Switching to profile: {}",
                            profile_name.display().to_string().bold()
                        );
                        switch_profile(&*profile_name.to_string_lossy())
                            .await
                            .wrap_err("Could not connect to the Eruption daemon")
                            .suggestion("Please verify that the Eruption daemon is running")?;
                    }

                    Err(_e) => {
                        eprintln!("No matches found");
                    }
                }
            }

            SwitchSubcommands::Slot { index } => {
                if index < 1 || index > constants::NUM_SLOTS {
                    eprintln!(
                        "Slot index out of bounds. Valid range is: {}-{}",
                        1,
                        constants::NUM_SLOTS
                    );
                } else {
                    println!("Switching to slot: {}", format!("{}", index).bold());
                    let index = index - 1;
                    switch_slot(index)
                        .await
                        .wrap_err("Could not connect to the Eruption daemon")
                        .suggestion("Please verify that the Eruption daemon is running")?;
                }
            }
        },

        Subcommands::Completions { shell } => {
            const BIN_NAME: &str = env!("CARGO_PKG_NAME");

            let mut command = Options::command();
            let mut fd = std::io::stdout();

            clap_complete::generate(shell, &mut command, BIN_NAME.to_string(), &mut fd);
        }
    };

    Ok(())
}

/// Main program entrypoint
pub fn main() -> std::result::Result<(), eyre::Error> {
    let language_loader: FluentLanguageLoader = fluent_language_loader!();

    let requested_languages = DesktopLanguageRequester::requested_languages();
    i18n_embed::select(&language_loader, &Localizations, &requested_languages)?;

    STATIC_LOADER.lock().replace(language_loader);

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    runtime.block_on(async move { async_main().await })
}
