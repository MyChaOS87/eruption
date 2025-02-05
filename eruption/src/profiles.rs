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

use crate::constants;
use log::*;
use paste::paste;
use serde::{Deserialize, Serialize};
use std::default::Default;
use std::fs;
use std::path::{Path, PathBuf};
use std::{collections::HashMap, ffi::OsStr};
use uuid::Uuid;

pub type Result<T> = std::result::Result<T, eyre::Error>;

#[derive(Debug, thiserror::Error)]
pub enum ProfileError {
    #[error("Could not open profile file for reading")]
    OpenError {},

    #[error("Could not parse profile file")]
    ParseError {},

    #[error("Could not save profile file: {msg}")]
    WriteError { msg: String },

    #[error("Could not find profile file from UUID")]
    FindError {},

    #[error("Could not enumerate profile files")]
    EnumError {},

    #[error("Could not set a config value in a profile: {msg}")]
    SetValueError { msg: String },

    #[error("Could not parse a param value")]
    ParseParamError {},
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, PartialOrd)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ConfigParam {
    Int {
        name: String,
        value: i64,
        #[serde(default)]
        default: i64,
    },
    Float {
        name: String,
        value: f64,
        #[serde(default)]
        default: f64,
    },
    Bool {
        name: String,
        value: bool,
        #[serde(default)]
        default: bool,
    },
    String {
        name: String,
        value: String,
        #[serde(default)]
        default: String,
    },
    Color {
        name: String,
        value: u32,
        #[serde(default)]
        default: u32,
    },
}

pub trait GetAttr {
    fn get_name(&self) -> &String;
    fn get_value(&self) -> String;
    fn get_default(&self) -> String;
}

impl GetAttr for ConfigParam {
    fn get_name(&self) -> &String {
        match self {
            ConfigParam::Int { ref name, .. } => name,

            ConfigParam::Float { ref name, .. } => name,

            ConfigParam::Bool { ref name, .. } => name,

            ConfigParam::String { ref name, .. } => name,

            ConfigParam::Color { ref name, .. } => name,
        }
    }

    fn get_value(&self) -> String {
        match self {
            ConfigParam::Int { ref value, .. } => format!("{}", value),

            ConfigParam::Float { ref value, .. } => format!("{}", value),

            ConfigParam::Bool { ref value, .. } => format!("{}", value),

            ConfigParam::String { ref value, .. } => value.to_owned(),

            ConfigParam::Color { ref value, .. } => format!("#{:06x}", value),
        }
    }

    fn get_default(&self) -> String {
        match self {
            ConfigParam::Int { ref default, .. } => format!("{}", default),

            ConfigParam::Float { ref default, .. } => format!("{}", default),

            ConfigParam::Bool { ref default, .. } => format!("{}", default),

            ConfigParam::String { ref default, .. } => default.to_owned(),

            ConfigParam::Color { ref default, .. } => format!("#{:06x}", default),
        }
    }
}

fn default_id() -> Uuid {
    Uuid::new_v4()
}

fn default_profile_file() -> PathBuf {
    "".into()
}

fn default_script_file() -> Vec<PathBuf> {
    vec![constants::DEFAULT_EFFECT_SCRIPT.into()]
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Profile {
    #[serde(default = "default_id")]
    pub id: Uuid,

    #[serde(default = "default_profile_file")]
    #[serde(skip_serializing)]
    pub profile_file: PathBuf,

    pub name: String,
    pub description: String,

    #[serde(default = "default_script_file")]
    pub active_scripts: Vec<PathBuf>,

    pub config: Option<HashMap<String, Vec<ConfigParam>>>,
}

macro_rules! get_default_value {
    ($t:ident, $tval:ty, $rval:ty) => {
        paste! {
            pub fn [<get_default_ $t>] (&self, script_name: &str, name: &str) -> Option<$rval> {
                match self.config.as_ref() {
                    Some(config) =>
                        match config.get(&script_name.to_owned()) {
                        Some(script_config) =>
                            match script_config.find_config_param(&name) {
                                Some(p) => match p {
                                    $tval {
                                        name: _,
                                        value: _,
                                        default,
                                    } => Some(default.clone()),

                                    _ => None,
                                },

                                _ => None,
                            },

                        _ => None,
                    }

                    _ => None,
                }
            }
        }
    };
}

impl Profile {
    // instantiate default value getters
    get_default_value!(int, ConfigParam::Int, i64);
    get_default_value!(float, ConfigParam::Float, f64);
    get_default_value!(bool, ConfigParam::Bool, bool);
    get_default_value!(string, ConfigParam::String, String);
    get_default_value!(color, ConfigParam::Color, u32);
}

pub trait FindConfig {
    fn find_config_param(&self, param: &str) -> Option<&ConfigParam>;
    fn find_config_param_mut(&mut self, param: &str) -> Option<&mut ConfigParam>;
}

impl FindConfig for Vec<ConfigParam> {
    fn find_config_param(&self, param: &str) -> Option<&ConfigParam> {
        for p in self.iter() {
            match p {
                ConfigParam::Int { name, .. } => {
                    if name == param {
                        return Some(p);
                    }
                }

                ConfigParam::Float { name, .. } => {
                    if name == param {
                        return Some(p);
                    }
                }

                ConfigParam::Bool { name, .. } => {
                    if name == param {
                        return Some(p);
                    }
                }

                ConfigParam::String { name, .. } => {
                    if name == param {
                        return Some(p);
                    }
                }

                ConfigParam::Color { name, .. } => {
                    if name == param {
                        return Some(p);
                    }
                }
            }
        }

        None
    }

    fn find_config_param_mut(&mut self, param: &str) -> Option<&mut ConfigParam> {
        for p in self.iter_mut() {
            match p {
                ConfigParam::Int { name, .. } => {
                    if name == param {
                        return Some(p);
                    }
                }

                ConfigParam::Float { name, .. } => {
                    if name == param {
                        return Some(p);
                    }
                }

                ConfigParam::Bool { name, .. } => {
                    if name == param {
                        return Some(p);
                    }
                }

                ConfigParam::String { name, .. } => {
                    if name == param {
                        return Some(p);
                    }
                }

                ConfigParam::Color { name, .. } => {
                    if name == param {
                        return Some(p);
                    }
                }
            }
        }

        None
    }
}

macro_rules! get_config_value {
    ($t:ident, $tval:ty, $pval:ty) => {
        paste::item! {
            pub fn [<get_ $t _value>](&self, script_name: &str, name: &str) -> Option<&$tval> {
                if let Some(config) = &self.config {
                    if let Some(cfg) = config.get(script_name) {
                        match cfg.find_config_param(name) {
                            Some(param) => match param {
                                $pval { value, .. } =>
                                {
                                    debug!("Using value from .profile file for config param '{}' (value: '{}') [5]",  name, value);

                                    Some(value)
                                }

                                _ => {
                                    debug!("Using default value for config param '{}' [4]", name);

                                    None
                                }
                            },

                            None => {
                                debug!("Using default value for config param '{}' [3]", name);

                                None
                            }
                        }
                    } else {
                        debug!("Using default value for config param [2]");

                        None
                    }
                } else {
                    debug!("Using default value for config param [1]");

                    None
                }
            }
        }
    };
}

macro_rules! set_config_value {
    ($t:ident, $tval:ty, $pval:ty) => {
        paste::item! {
            pub fn [<set_ $t _value>](&mut self, script_name: &str, name: &str, val: &$tval) -> Result<()> {
                if let Some(ref mut config) = self.config {
                    if let Some(ref mut cfg) = config.get_mut(script_name) {
                        match cfg.find_config_param_mut(name) {
                            Some(ref mut param) => match param {
                                $pval { ref mut value, .. } => {
                                    *value = val.to_owned();
                                    Ok(())
                                }

                                _ => Err(ProfileError::SetValueError {
                                    msg: "Invalid data type".into(),
                                }.into()),
                            },

                            _ => {
                                cfg.push($pval {
                                    name: name.to_string(),
                                    value: val.to_owned(),
                                    default: val.to_owned(),
                                });
                                Ok(())
                            }
                        }
                    } else {
                        config.insert(
                            script_name.into(),
                            vec![$pval {
                                name: name.to_string(),
                                value: val.to_owned(),
                                default: val.to_owned(),
                            }],
                        );

                        Ok(())
                    }
                } else {
                    Err(ProfileError::SetValueError {
                        msg: "Could not get config".into(),
                    }.into())
                }
            }
        }
    };
}

impl Profile {
    pub fn new(profile_file: &Path) -> Result<Self> {
        // parse manifest
        match fs::read_to_string(profile_file) {
            Ok(toml) => {
                // parse profile
                match toml::de::from_str::<Self>(&toml) {
                    Ok(mut result) => {
                        // fill in required fields, after parsing
                        result.id = Uuid::new_v4();
                        result.profile_file = profile_file.to_path_buf();

                        if result.config.is_none() {
                            result.config = Some(HashMap::new());
                        }

                        Ok(result)
                    }

                    Err(_e) => Err(ProfileError::ParseError {}.into()),
                }
            }

            Err(_e) => Err(ProfileError::OpenError {}.into()),
        }
    }

    /// Returns a failsafe profile that will work in almost all cases
    pub fn new_fail_safe() -> Self {
        Self {
            id: Uuid::new_v4(),
            name: "Failsafe mode".to_string(),
            description: "Failsafe mode virtual profile".to_string(),
            profile_file: PathBuf::from("failsafe.profile"),
            active_scripts: vec![PathBuf::from("lib/failsafe.lua")],
            ..Default::default()
        }
    }

    pub fn from(profile_file: &Path) -> Result<Self> {
        // parse manifest
        match fs::read_to_string(profile_file) {
            Ok(toml) => {
                // parse profile
                match toml::de::from_str::<Self>(&toml) {
                    Ok(mut result) => {
                        // fill in required fields, after parsing
                        result.profile_file = profile_file.to_path_buf();

                        // load persisted profile state from disk, but ignore errors
                        let _ = result
                            .load_params()
                            .map_err(|e| trace!("Error loading profile state from disk: {}", e));

                        if result.config.is_none() {
                            result.config = Some(HashMap::new());
                        }

                        Ok(result)
                    }

                    Err(_e) => Err(ProfileError::ParseError {}.into()),
                }
            }

            Err(_e) => Err(ProfileError::OpenError {}.into()),
        }
    }

    pub fn find_by_uuid(uuid: Uuid) -> Result<Self> {
        let mut result = Err(ProfileError::FindError {}.into());

        if let Ok(profile_files) = get_profile_files() {
            'PROFILE_LOOP: for profile_file in profile_files.iter() {
                match Profile::from(profile_file) {
                    Ok(profile) => {
                        if profile.id == uuid {
                            result = Ok(profile);
                            break 'PROFILE_LOOP;
                        }
                    }

                    Err(e) => {
                        error!(
                            "Could not process profile {}: {}",
                            profile_file.display(),
                            e
                        );
                    }
                }
            }
        }

        result
    }

    pub fn save(&self) -> Result<()> {
        let toml = toml::ser::to_string_pretty(&self)?;

        fs::write(&self.profile_file, &toml).map_err(|_| ProfileError::WriteError {
            msg: "Could not write file".into(),
        })?;

        Ok(())
    }

    pub fn load_params(&mut self) -> Result<()> {
        let path = self.profile_file.with_extension("profile.state");
        let json_string = fs::read_to_string(&path)?;

        let map: HashMap<String, Vec<ConfigParam>> = serde_json::from_str(&json_string)?;

        self.config = Some(map);

        Ok(())
    }

    pub fn save_params(&self) -> Result<()> {
        if let Some(ref config) = self.config {
            let json_string = serde_json::to_string_pretty(&config)?;
            let path = self.profile_file.with_extension("profile.state");

            fs::write(&path, json_string)?;
        }

        Ok(())
    }

    get_config_value!(int, i64, ConfigParam::Int);
    set_config_value!(int, i64, ConfigParam::Int);

    get_config_value!(float, f64, ConfigParam::Float);
    set_config_value!(float, f64, ConfigParam::Float);

    get_config_value!(bool, bool, ConfigParam::Bool);
    set_config_value!(bool, bool, ConfigParam::Bool);

    get_config_value!(string, str, ConfigParam::String);
    set_config_value!(string, str, ConfigParam::String);

    get_config_value!(color, u32, ConfigParam::Color);
    set_config_value!(color, u32, ConfigParam::Color);
}

impl Default for Profile {
    fn default() -> Self {
        let profile_file =
            Path::new(constants::DEFAULT_PROFILE_DIR).join(Path::new("default.profile"));

        let config = Some(HashMap::new());

        Self {
            id: default_id(),
            profile_file,
            name: "Default".into(),
            description: "Auto-generated profile".into(),
            active_scripts: vec![PathBuf::from(constants::DEFAULT_EFFECT_SCRIPT)],
            config,
        }
    }
}

pub fn get_profile_dirs() -> Vec<PathBuf> {
    let mut result = vec![];

    let config = crate::CONFIG.lock();

    let profile_dirs = config
        .as_ref()
        .unwrap()
        .get::<Vec<String>>("global.profile_dirs")
        .unwrap_or_else(|_| vec![]);

    let mut profile_dirs = profile_dirs
        .iter()
        .map(PathBuf::from)
        .collect::<Vec<PathBuf>>();

    result.append(&mut profile_dirs);

    // if we could not determine a valid set of paths, use a hard coded fallback instead
    if result.is_empty() {
        log::warn!("Using default fallback profile directory");

        let path = PathBuf::from(constants::DEFAULT_PROFILE_DIR);
        result.push(path);
    }

    result
}

pub fn get_profiles() -> Result<Vec<Profile>> {
    get_profiles_from(&get_profile_dirs())
}

pub fn get_profiles_from(profile_dirs: &[PathBuf]) -> Result<Vec<Profile>> {
    let mut result: Vec<Profile> = vec![];
    let mut errors_present = false;

    let profile_files = get_profile_files_from(profile_dirs).unwrap_or_else(|e| {
        log::warn!("Could not enumerate profiles: {}", &e);
        vec![]
    });

    for profile_file in profile_files.iter() {
        match Profile::from(profile_file) {
            Ok(profile) => {
                result.push(profile);
            }

            Err(e) => {
                errors_present = true;
                error!(
                    "Could not process profile {}: {}",
                    profile_file.display(),
                    e
                );
            }
        }
    }

    if errors_present {
        warn!("An error occurred during processing of profiles");
    }

    Ok(result)
}

pub fn get_profile_files() -> Result<Vec<PathBuf>> {
    get_profile_files_from(&get_profile_dirs())
}

pub fn get_profile_files_from(profile_dirs: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let mut result = vec![];

    for profile_path in profile_dirs {
        if let Ok(paths) = fs::read_dir(&profile_path) {
            let mut profile_paths = paths
                .map(|p| p.unwrap().path())
                .filter(|p| {
                    if p.extension().is_some() {
                        return p.extension().unwrap_or_else(|| OsStr::new("")) == "profile";
                    }

                    false
                })
                .collect::<Vec<PathBuf>>();

            result.append(&mut profile_paths);
        }
    }

    Ok(result)
}

pub fn find_path_by_uuid(uuid: Uuid) -> Option<PathBuf> {
    find_path_by_uuid_from(uuid, &get_profile_dirs())
}

pub fn find_path_by_uuid_from(uuid: Uuid, profile_dirs: &Vec<PathBuf>) -> Option<PathBuf> {
    let profile_files = get_profile_files_from(profile_dirs).unwrap_or_else(|_| vec![]);

    let mut errors_present = false;
    let mut result = None;

    'PROFILE_LOOP: for profile_file in profile_files.iter() {
        match Profile::from(profile_file) {
            Ok(profile) => {
                if profile.id == uuid {
                    result = Some(profile_file.to_path_buf());
                    break 'PROFILE_LOOP;
                }
            }

            Err(e) => {
                errors_present = true;
                error!(
                    "Could not process profile {}: {}",
                    profile_file.display(),
                    e
                );
            }
        }
    }

    if errors_present {
        warn!("An error occurred during processing of profiles");
    }

    result
}

pub fn get_fail_safe_profile() -> Profile {
    Profile::new_fail_safe()
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, str::FromStr};

    use uuid::Uuid;

    use super::FindConfig;

    #[test]
    fn enum_profile_files() -> super::Result<()> {
        let path = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
        let files = super::get_profile_files_from(&[path.join("../support/tests/assets/")])?;

        assert!(
            files.contains(&path.join("../support/tests/assets/default.profile")),
            "Missing default.profile: {:#?}",
            files
        );

        Ok(())
    }

    #[test]
    fn enum_profiles() -> super::Result<()> {
        let path = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
        let profiles = super::get_profiles_from(&[path.join("../support/tests/assets/")])?;

        assert!(
            profiles
                .iter()
                .map(|p| p.name.as_ref())
                .collect::<Vec<&str>>()
                .contains(&"Organic FX"),
            "Missing profile 'Organic FX' in profiles: {:#?}",
            profiles
        );

        Ok(())
    }

    #[test]
    fn find_profile_path_by_uuid() -> super::Result<()> {
        let uuid = Uuid::from_str("5dc62fa6-e965-45cb-a0da-e87d29713093").unwrap();

        let path = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
        let profile_path =
            super::find_path_by_uuid_from(uuid, &vec![path.join("../support/tests/assets/")])
                .unwrap();

        assert_eq!(
            profile_path,
            path.join("../support/tests/assets/default.profile"),
            "Invalid path {:#?}",
            profile_path
        );

        Ok(())
    }

    #[test]
    fn load_profile_by_path() -> super::Result<()> {
        let uuid = Uuid::from_str("5dc62fa6-e965-45cb-a0da-e87d29713093").unwrap();

        let path = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
        let profile_path =
            super::find_path_by_uuid_from(uuid, &vec![path.join("../support/tests/assets/")])
                .unwrap();

        let profile = super::Profile::from(&profile_path)?;

        assert_eq!(profile.id, uuid);
        assert_eq!(profile.name, "Organic FX");

        Ok(())
    }

    #[test]
    fn test_profile_parameters() -> super::Result<()> {
        let uuid = Uuid::from_str("5dc62fa6-e965-45cb-a0da-e87d29713093").unwrap();

        let path = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
        let profile_path =
            super::find_path_by_uuid_from(uuid, &vec![path.join("../support/tests/assets/")])
                .unwrap();

        let profile = super::Profile::from(&profile_path)?;

        assert_eq!(profile.id, uuid);
        assert_eq!(profile.name, "Organic FX");

        let config = profile.config.unwrap();

        let param = config["Shockwave"]
            .find_config_param("color_step_shockwave")
            .unwrap();

        assert_eq!(
            param,
            &super::ConfigParam::Color {
                name: String::from("color_step_shockwave"),
                value: 0x05010000,
                default: Default::default(),
            }
        );

        let param = config["Shockwave"]
            .find_config_param("mouse_events")
            .unwrap();

        assert_eq!(
            param,
            &super::ConfigParam::Bool {
                name: String::from("mouse_events"),
                value: true,
                default: Default::default(),
            }
        );

        Ok(())
    }
}
