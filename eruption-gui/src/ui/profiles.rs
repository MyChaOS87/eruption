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

use crate::{dbus_client, profiles::FindConfig};
use crate::{
    manifest,
    profiles::{self, Profile},
};
use crate::{manifest::Manifest, util};
use glib::clone;
use glib::IsA;
use gtk::builders::{
    AdjustmentBuilder, BoxBuilder, ButtonBuilder, ColorChooserWidgetBuilder, EntryBuilder,
    ExpanderBuilder, FrameBuilder, LabelBuilder, MessageDialogBuilder, ScaleBuilder,
    ScrolledWindowBuilder, SwitchBuilder, TreeViewColumnBuilder,
};
use gtk::glib;
use gtk::{
    prelude::*, Align, Builder, ButtonsType, CellRendererText, IconSize, Image, Justification,
    MessageType, Orientation, PositionType, ScrolledWindow, Stack, StackSwitcher, TextBuffer,
    TreeStore, TreeView, TreeViewColumnSizing,
};
use gtk::{Frame, ShadowType};
use paste::paste;

#[cfg(feature = "sourceview")]
use gtk::TextView;
#[cfg(feature = "sourceview")]
use sourceview4::builders::BufferBuilder;
#[cfg(feature = "sourceview")]
use sourceview4::prelude::*;

#[cfg(not(feature = "sourceview"))]
use gtk::builders::{TextBufferBuilder, TextViewBuilder};
#[cfg(not(feature = "sourceview"))]
use gtk::ApplicationWindow;

use std::path::{Path, PathBuf};
use std::{cell::RefCell, collections::HashMap, ffi::OsStr, rc::Rc};

type Result<T> = std::result::Result<T, eyre::Error>;

cfg_if::cfg_if! {
    if #[cfg(feature = "sourceview")] {
        thread_local! {
            /// Holds the source code buffers and the respective paths in the file system
            static TEXT_BUFFERS: Rc<RefCell<HashMap<PathBuf, (usize, sourceview4::Buffer)>>> = Rc::new(RefCell::new(HashMap::new()));
        }
    } else {
        thread_local! {
            /// Holds the source code buffers and the respective paths in the file system
            static TEXT_BUFFERS: Rc<RefCell<HashMap<PathBuf, (usize, TextBuffer)>>> = Rc::new(RefCell::new(HashMap::new()));
        }
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum ProfilesError {
    #[error("Unknown error: {description}")]
    UnknownError { description: String },

    #[error("Parameter has an invalid data type")]
    TypeMismatch {},

    #[error("Method call failed: {description}")]
    MethodCallError { description: String },
}

macro_rules! declare_config_widget_numeric {
    (i64) => {
        paste! {
            fn [<build_config_widget_ i64>] <F: Fn(i64) + 'static>(
                name: &str,
                description: &str,
                default: i64,
                min: Option<i64>,
                max: Option<i64>,
                value:i64,
                callback: F,
            ) -> Result<gtk::Box> {
                let container = BoxBuilder::new()
                    .border_width(16)
                    .halign(Align::Fill)
                    .valign(Align::Fill)
                    .orientation(Orientation::Vertical)
                    .homogeneous(false)
                    .build();

                let row1 = BoxBuilder::new()
                    .halign(Align::Fill)
                    .valign(Align::Fill)
                    .spacing(8)
                    .orientation(Orientation::Horizontal)
                    .homogeneous(false)
                    .build();

                container.pack_start(&row1, true, true, 8);

                let row2 = BoxBuilder::new()
                    .halign(Align::Fill)
                    .valign(Align::Fill)
                    .spacing(8)
                    .orientation(Orientation::Horizontal)
                    .homogeneous(false)
                    .build();

                container.pack_start(&row2, true, true, 8);

                let label = LabelBuilder::new()
                    .expand(false)
                    .halign(Align::Start)
                    .justify(Justification::Left)
                    .use_markup(true)
                    .label(&format!("<b>{}</b>", name))
                    .build();

                row1.pack_start(&label, false, false, 8);

                let label = LabelBuilder::new()
                    .expand(false)
                    .halign(Align::Start)
                    .justify(Justification::Left)
                    .label(&description)
                    .build();

                row1.pack_start(&label, false, false, 8);

                // "reset to default value" button
                let image = Image::from_icon_name(Some("reload"), IconSize::Button);
                let reset_button = ButtonBuilder::new()
                    .halign(Align::Start)
                    .image(&image)
                    .tooltip_text("Reset this parameter to its default value")
                    .build();

                row2.pack_start(&reset_button, false, false, 8);

                // scale widget
                // set constraints
                let mut adjustment = AdjustmentBuilder::new();

                adjustment = adjustment.value(value as f64);
                adjustment = adjustment.step_increment(1.0);

                if let Some(min) = min {
                    adjustment = adjustment.lower(min as f64);
                }

                if let Some(max) = max {
                    adjustment = adjustment.upper(max as f64);
                }

                let adjustment = adjustment.build();

                let scale = ScaleBuilder::new()
                    .halign(Align::Fill)
                    .hexpand(true)
                    .adjustment(&adjustment)
                    .digits(0)
                    .value_pos(PositionType::Left)
                    .build();

                row2.pack_start(&scale, false, true, 8);

                scale.connect_value_changed(move |c| {
                    let value = c.value() as i64;
                    callback(value);
                });

                reset_button.connect_clicked(clone!(@weak adjustment => move |_b| {
                    adjustment.set_value(default as f64);
                }));

                Ok(container)
            }
        }
    };

    ($t:ty) => {
        paste! {
            fn [<build_config_widget_ $t>] <F: Fn($t) + 'static>(
                name: &str,
                description: &str,
                default: $t,
                min: Option<$t>,
                max: Option<$t>,
                value: $t,
                callback: F,
            ) -> Result<gtk::Box> {
                let container = BoxBuilder::new()
                    .border_width(16)
                    .halign(Align::Fill)
                    .valign(Align::Fill)
                    .orientation(Orientation::Vertical)
                    .homogeneous(false)
                    .build();

                let row1 = BoxBuilder::new()
                    .halign(Align::Fill)
                    .valign(Align::Fill)
                    .spacing(8)
                    .orientation(Orientation::Horizontal)
                    .homogeneous(false)
                    .build();

                container.pack_start(&row1, true, true, 8);

                let row2 = BoxBuilder::new()
                    .halign(Align::Fill)
                    .valign(Align::Fill)
                    .spacing(8)
                    .orientation(Orientation::Horizontal)
                    .homogeneous(false)
                    .build();

                container.pack_start(&row2, true, true, 8);

                let label = LabelBuilder::new()
                    .expand(false)
                    .halign(Align::Start)
                    .justify(Justification::Left)
                    .use_markup(true)
                    .label(&format!("<b>{}</b>", name))
                    .build();

                row1.pack_start(&label, false, false, 8);

                let label = LabelBuilder::new()
                    .expand(false)
                    .halign(Align::Start)
                    .justify(Justification::Left)
                    .label(&description)
                    .build();

                row1.pack_start(&label, false, false, 8);

                // "reset to default value" button
                let image = Image::from_icon_name(Some("reload"), IconSize::Button);
                let reset_button = ButtonBuilder::new()
                    .halign(Align::Start)
                    .image(&image)
                    .tooltip_text("Reset this parameter to its default value")
                    .build();

                row2.pack_start(&reset_button, false, false, 8);

                // scale widget
                // set constraints
                let mut adjustment = AdjustmentBuilder::new();

                adjustment = adjustment.value(value as f64);
                adjustment = adjustment.step_increment(0.01);

                if let Some(min) = min {
                    adjustment = adjustment.lower(min as f64);
                }

                if let Some(max) = max {
                    adjustment = adjustment.upper(max as f64);
                }

                let adjustment = adjustment.build();

                let scale = ScaleBuilder::new()
                    .halign(Align::Fill)
                    .hexpand(true)
                    .adjustment(&adjustment)
                    .digits(2)
                    .value_pos(PositionType::Left)
                    .build();

                row2.pack_start(&scale, false, true, 8);

                scale.connect_value_changed(move |c| {
                    let value = c.value() as $t;
                    callback(value);
                });

                reset_button.connect_clicked(clone!(@weak adjustment => move |_b| {
                    adjustment.set_value(default as f64);
                }));

                Ok(container)
            }
        }
    };
}

macro_rules! declare_config_widget_input {
    ($t:ty) => {
        paste! {
            fn [<build_config_widget_input_ $t:lower>] <F: Fn($t) + 'static>(
                name: &str,
                description: &str,
                default: String,
                value: String,
                callback: F,
            ) -> Result<gtk::Box> {
                let container = BoxBuilder::new()
                    .border_width(16)
                    .halign(Align::Fill)
                    .valign(Align::Fill)
                    .orientation(Orientation::Vertical)
                    .homogeneous(false)
                    .build();

                let row1 = BoxBuilder::new()
                    .halign(Align::Fill)
                    .valign(Align::Fill)
                    .spacing(8)
                    .orientation(Orientation::Horizontal)
                    .homogeneous(false)
                    .build();

                container.pack_start(&row1, true, true, 8);

                let row2 = BoxBuilder::new()
                    .halign(Align::Fill)
                    .valign(Align::Fill)
                    .spacing(8)
                    .orientation(Orientation::Horizontal)
                    .homogeneous(false)
                    .build();

                container.pack_start(&row2, true, true, 8);

                let label = LabelBuilder::new()
                    .expand(false)
                    .halign(Align::Start)
                    .justify(Justification::Left)
                    .use_markup(true)
                    .label(&format!("<b>{}</b>", name))
                    .build();

                row1.pack_start(&label, false, false, 8);

                let label = LabelBuilder::new()
                    .expand(false)
                    .halign(Align::Start)
                    .justify(Justification::Left)
                    .label(&description)
                    .build();

                row1.pack_start(&label, false, false, 8);

                // "reset to default value" button
                let image = Image::from_icon_name(Some("reload"), IconSize::Button);
                let reset_button = ButtonBuilder::new()
                    .halign(Align::Start)
                    .image(&image)
                    .tooltip_text("Reset this parameter to its default value")
                    .build();

                row2.pack_start(&reset_button, false, false, 8);

                // entry widget
                let entry = EntryBuilder::new().text(&value).build();

                row2.pack_start(&entry, false, true, 8);

                entry.connect_changed(move |e| {
                    let value = e.text();
                    callback(value.to_string());
                });

                reset_button.connect_clicked(clone!(@weak entry, @strong default => move |_b| {
                    entry.set_text(&default);
                }));

                Ok(container)
            }
        }
    };
}

macro_rules! declare_config_widget_color {
    ($t:ty) => {
        paste! {
            fn [<build_config_widget_color_ $t>] <F: Clone + Fn($t) + 'static>(
                name: &str,
                description: &str,
                default: $t,
                _min: Option<$t>,
                _max: Option<$t>,
                value: $t,
                callback: F,
            ) -> Result<gtk::Box> {
                let container = BoxBuilder::new()
                    .border_width(16)
                    .halign(Align::Fill)
                    .valign(Align::Fill)
                    .orientation(Orientation::Vertical)
                    .homogeneous(false)
                    .build();

                let row1 = BoxBuilder::new()
                    .halign(Align::Fill)
                    .valign(Align::Fill)
                    .spacing(8)
                    .orientation(Orientation::Horizontal)
                    .homogeneous(false)
                    .build();

                container.pack_start(&row1, true, true, 8);

                let row2 = BoxBuilder::new()
                    .halign(Align::Fill)
                    .valign(Align::Fill)
                    .spacing(8)
                    .orientation(Orientation::Horizontal)
                    .homogeneous(false)
                    .build();

                container.pack_start(&row2, true, true, 8);

                let label = LabelBuilder::new()
                    .expand(false)
                    .halign(Align::Start)
                    .justify(Justification::Left)
                    .use_markup(true)
                    .label(&format!("<b>{}</b>", name))
                    .build();

                row1.pack_start(&label, false, false, 8);

                let label = LabelBuilder::new()
                    .expand(false)
                    .halign(Align::Start)
                    .justify(Justification::Left)
                    .label(&description)
                    .build();

                row1.pack_start(&label, false, false, 8);

                // "reset to default value" button
                let image = Image::from_icon_name(Some("reload"), IconSize::Button);
                let reset_button = ButtonBuilder::new()
                    .halign(Align::Start)
                    .image(&image)
                    .tooltip_text("Reset this parameter to its default value")
                    .build();

                row2.pack_start(&reset_button, false, false, 8);

                // color chooser widget
                let rgba = util::color_to_gdk_rgba(value);
                let chooser = ColorChooserWidgetBuilder::new()
                    .rgba(&rgba)
                    .use_alpha(true)
                    .show_editor(false)
                    .build();

                row2.pack_start(&chooser, false, true, 8);

                chooser.connect_color_activated(clone!(@strong callback => move |_c, color| {
                    let value = util::gdk_rgba_to_color(color);
                    callback(value);
                }));

                reset_button.connect_clicked(clone!(@strong callback, @strong chooser => move |_b| {
                    chooser.set_rgba(&util::color_to_gdk_rgba(default));
                    callback(default);
                }));

                Ok(container)
            }
        }
    };
}

macro_rules! declare_config_widget_switch {
    ($t:ty) => {
        paste! {
            fn [<build_config_widget_switch_ $t>] <F: Fn($t) + 'static>(
                name: &str,
                description: &str,
                default: $t,
                value: $t,
                callback: F,
            ) -> Result<gtk::Box> {
                let container = BoxBuilder::new()
                    .border_width(16)
                    .halign(Align::Fill)
                    .valign(Align::Fill)
                    .orientation(Orientation::Vertical)
                    .homogeneous(false)
                    .build();

                let row1 = BoxBuilder::new()
                    .halign(Align::Fill)
                    .valign(Align::Fill)
                    .spacing(8)
                    .orientation(Orientation::Horizontal)
                    .homogeneous(false)
                    .build();

                container.pack_start(&row1, true, true, 8);

                let row2 = BoxBuilder::new()
                    .halign(Align::Fill)
                    .valign(Align::Fill)
                    .spacing(8)
                    .orientation(Orientation::Horizontal)
                    .homogeneous(false)
                    .build();

                container.pack_start(&row2, true, true, 8);

                let label = LabelBuilder::new()
                    .expand(false)
                    .halign(Align::Start)
                    .justify(Justification::Left)
                    .use_markup(true)
                    .label(&format!("<b>{}</b>", name))
                    .build();

                row1.pack_start(&label, false, false, 8);

                let label = LabelBuilder::new()
                    .expand(false)
                    .halign(Align::Start)
                    .justify(Justification::Left)
                    .label(&description)
                    .build();

                row1.pack_start(&label, false, false, 8);

                // "reset to default value" button
                let image = Image::from_icon_name(Some("reload"), IconSize::Button);
                let reset_button = ButtonBuilder::new()
                    .halign(Align::Start)
                    .image(&image)
                    .tooltip_text("Reset this parameter to its default value")
                    .build();

                row2.pack_start(&reset_button, false, false, 8);

                // switch widget
                let switch = SwitchBuilder::new()
                    .expand(false)
                    .valign(Align::Center)
                    .state(value)
                    .build();

                row2.pack_start(&switch, false, false, 8);

                switch.connect_changed_active(move |s| {
                    let value = s.state();
                    callback(value);
                });

                reset_button.connect_clicked(clone!(@weak switch => move |_| {
                    switch.set_state(default);
                }));

                Ok(container)
            }
        }
    };
}

declare_config_widget_numeric!(i64);
declare_config_widget_numeric!(f64);

declare_config_widget_input!(String);
declare_config_widget_color!(u32);
declare_config_widget_switch!(bool);

fn create_config_editor(
    profile: &Profile,
    script: &Manifest,
    param: &manifest::ConfigParam,
    value: &Option<&profiles::ConfigParam>,
) -> Result<Frame> {
    fn parameter_changed<T>(profile: &Profile, script: &Manifest, name: &str, value: T)
    where
        T: std::fmt::Display,
    {
        log::debug!(
            "Setting parameter {}: {}: {} to '{}'",
            &profile.profile_file.display(),
            &script.script_file.display(),
            &name,
            &value
        );

        crate::dbus_client::set_parameter(
            &profile.profile_file.to_string_lossy(),
            &script.script_file.to_string_lossy(),
            &name,
            &format!("{}", &value),
        )
        .unwrap();
    }

    let outer = FrameBuilder::new()
        .border_width(16)
        // .label(&format!("{}", param.get_name()))
        // .label_xalign(0.0085)
        .build();

    match &param {
        manifest::ConfigParam::Int {
            name,
            description,
            min,
            max,
            default,
        } => {
            let value = if let Some(value) = value {
                match value {
                    profiles::ConfigParam::Int { name: _, value, .. } => *value,

                    _ => return Err(ProfilesError::TypeMismatch {}.into()),
                }
            } else {
                match param {
                    manifest::ConfigParam::Int {
                        name: _, default, ..
                    } => profile
                        .get_default_int(&script.name, &name)
                        .or_else(|| Some(*default))
                        .unwrap(),

                    _ => return Err(ProfilesError::TypeMismatch {}.into()),
                }
            };

            let default = profile
                .get_default_int(&script.name, &name)
                .or_else(|| Some(*default))
                .unwrap();

            let widget = build_config_widget_i64(
                &name,
                &description,
                default,
                *min,
                *max,
                value,
                clone!(@strong profile, @strong script, @strong name => move |value| {
                    parameter_changed(&profile, &script, &name, &value);
                }),
            )?;

            outer.add(&widget);
        }

        manifest::ConfigParam::Float {
            name,
            description,
            min,
            max,
            default,
        } => {
            let value = if let Some(value) = value {
                match value {
                    profiles::ConfigParam::Float { name: _, value, .. } => *value,

                    _ => return Err(ProfilesError::TypeMismatch {}.into()),
                }
            } else {
                match param {
                    manifest::ConfigParam::Float {
                        name: _, default, ..
                    } => profile
                        .get_default_float(&script.name, &name)
                        .or_else(|| Some(*default))
                        .unwrap(),

                    _ => return Err(ProfilesError::TypeMismatch {}.into()),
                }
            };

            let default = profile
                .get_default_float(&script.name, &name)
                .or_else(|| Some(*default))
                .unwrap();

            let widget = build_config_widget_f64(
                &name,
                &description,
                default,
                *min,
                *max,
                value,
                clone!(@strong profile, @strong script, @strong name => move |value| {
                    parameter_changed(&profile, &script, &name, &value);
                }),
            )?;

            outer.add(&widget);
        }

        manifest::ConfigParam::Bool {
            name,
            description,
            default,
        } => {
            let value = if let Some(value) = value {
                match value {
                    profiles::ConfigParam::Bool { name: _, value, .. } => *value,

                    _ => return Err(ProfilesError::TypeMismatch {}.into()),
                }
            } else {
                match param {
                    manifest::ConfigParam::Bool {
                        name: _, default, ..
                    } => profile
                        .get_default_bool(&script.name, &name)
                        .or_else(|| Some(*default))
                        .unwrap(),

                    _ => return Err(ProfilesError::TypeMismatch {}.into()),
                }
            };

            let default = profile
                .get_default_bool(&script.name, &name)
                .or_else(|| Some(*default))
                .unwrap();

            let widget = build_config_widget_switch_bool(
                &name,
                &description,
                default,
                value,
                clone!(@strong profile, @strong script, @strong name => move |value| {
                    parameter_changed(&profile, &script, &name, &value);
                }),
            )?;

            outer.add(&widget);
        }

        manifest::ConfigParam::String {
            name,
            description,
            default,
        } => {
            let value = if let Some(value) = *value {
                match value {
                    profiles::ConfigParam::String { name: _, value, .. } => value.clone(),

                    _ => return Err(ProfilesError::TypeMismatch {}.into()),
                }
            } else {
                match param {
                    manifest::ConfigParam::String {
                        name: _, default, ..
                    } => profile
                        .get_default_string(&script.name, &name)
                        .or_else(|| Some(default.clone()))
                        .unwrap()
                        .to_owned(),
                    _ => return Err(ProfilesError::TypeMismatch {}.into()),
                }
            };

            let default = profile
                .get_default_string(&script.name, &name)
                .or_else(|| Some(default.clone()))
                .unwrap();

            let widget = build_config_widget_input_string(
                &name,
                &description,
                default,
                value,
                clone!(@strong profile, @strong script, @strong name => move |value| {
                    parameter_changed(&profile, &script, &name, &value);
                }),
            )?;

            outer.add(&widget);
        }

        manifest::ConfigParam::Color {
            name,
            description,
            min,
            max,
            default,
        } => {
            let value = if let Some(value) = value {
                match value {
                    profiles::ConfigParam::Color { name: _, value, .. } => *value,

                    _ => return Err(ProfilesError::TypeMismatch {}.into()),
                }
            } else {
                match param {
                    manifest::ConfigParam::Color {
                        name: _, default, ..
                    } => profile
                        .get_default_color(&script.name, &name)
                        .or_else(|| Some(*default))
                        .unwrap(),

                    _ => return Err(ProfilesError::TypeMismatch {}.into()),
                }
            };

            let default = profile
                .get_default_color(&script.name, &name)
                .or_else(|| Some(*default))
                .unwrap();

            let widget = build_config_widget_color_u32(
                &name,
                &description,
                default,
                *min,
                *max,
                value,
                clone!(@strong profile, @strong script, @strong name => move |value| {
                    parameter_changed(&profile, &script, &name, &value);
                }),
            )?;

            outer.add(&widget);
        }
    }

    Ok(outer)
}

/// Populate the configuration tab with settings/GUI controls
fn populate_visual_config_editor<P: AsRef<Path>>(builder: &Builder, profile: P) -> Result<()> {
    let config_window: ScrolledWindow = builder.object("config_window").unwrap();

    // first, clear all child widgets
    config_window.foreach(|widget| {
        config_window.remove(widget);
    });

    // then add config items
    let container = BoxBuilder::new()
        .border_width(8)
        .orientation(Orientation::Vertical)
        .spacing(8)
        .homogeneous(false)
        .build();

    let profile = Profile::from(profile.as_ref())?;

    let label = LabelBuilder::new()
        .label(&format!("{}", &profile.name,))
        .justify(Justification::Fill)
        .halign(Align::Start)
        .build();

    let context = label.style_context();
    context.add_class("heading");

    container.pack_start(&label, false, false, 8);

    for f in &profile.active_scripts {
        let manifest = Manifest::from(&util::match_script_file(f)?)?;

        let expander = ExpanderBuilder::new()
            .border_width(8)
            .label(&format!("{} ({})", &manifest.name, &f.display()))
            .build();

        let expander_frame = FrameBuilder::new()
            .border_width(8)
            .shadow_type(ShadowType::None)
            .build();

        let expander_container = BoxBuilder::new()
            .orientation(Orientation::Vertical)
            .homogeneous(false)
            .build();

        expander_frame.add(&expander_container);
        expander.add(&expander_frame);

        container.pack_start(&expander, false, false, 8);

        if let Some(params) = &manifest.config {
            for param in params {
                let name = match &param {
                    manifest::ConfigParam::Int { name, .. } => name,

                    manifest::ConfigParam::Float { name, .. } => name,

                    manifest::ConfigParam::Bool { name, .. } => name,

                    manifest::ConfigParam::String { name, .. } => name,

                    manifest::ConfigParam::Color { name, .. } => name,
                };

                let value = if let Some(ref values) = profile.config {
                    match values.get(name) {
                        Some(e) => e.find_config_param(name),

                        None => None,
                    }
                } else {
                    None
                };

                let child = create_config_editor(&profile, &manifest, param, &value)?;
                expander_container.pack_start(&child, false, true, 0);
            }
        }
    }

    config_window.add(&container);
    config_window.show_all();

    Ok(())
}

/// Remove unused elements from the profiles stack, except the "Configuration" page
fn remove_elements_from_stack_widget(builder: &Builder) {
    let stack_widget: Stack = builder.object("profile_stack").unwrap();

    stack_widget.foreach(|widget| {
        stack_widget.remove(widget);
    });

    TEXT_BUFFERS.with(|b| b.borrow_mut().clear());
}

cfg_if::cfg_if! {
    if #[cfg(feature = "sourceview")] {
        fn save_buffer_contents_to_file<P: AsRef<Path>>(
            path: &P,
            buffer: &sourceview4::Buffer,
            builder: &Builder,
        ) -> Result<()> {
            let main_window: gtk::ApplicationWindow = builder.object("main_window").unwrap();

            let buffer = buffer.dynamic_cast_ref::<TextBuffer>().unwrap();
            let (start, end) = buffer.bounds();
            let data = buffer.text(&start, &end, true).map(|v| v.to_string());

            match data {
                Some(data) => {
                    // log::debug!("{}", &data);

                    if let Err(e) = dbus_client::write_file(&path.as_ref(), &data) {
                        log::error!("{}", e);

                        let message = "Could not write file".to_string();
                        let secondary =
                            format!("Error writing to file {}: {}", &path.as_ref().display(), e);

                        let message_dialog = MessageDialogBuilder::new()
                            .parent(&main_window)
                            .destroy_with_parent(true)
                            .decorated(true)
                            .message_type(MessageType::Error)
                            .text(&message)
                            .secondary_text(&secondary)
                            .title("Error")
                            .buttons(ButtonsType::Ok)
                            .build();

                        message_dialog.run();
                        message_dialog.hide();

                        Err(ProfilesError::MethodCallError {
                            description: "Could not write file".to_string(),
                        }
                        .into())
                    } else {
                        log::info!("Wrote file: {}", &path.as_ref().display());

                        Ok(())
                    }
                }

                _ => {
                    log::error!("Could not get buffer contents");

                    Err(ProfilesError::UnknownError {
                        description: "Could not get buffer contents".to_string(),
                    }
                    .into())
                }
            }
        }
    } else {
        fn save_buffer_contents_to_file<P: AsRef<Path>>(
            path: &P,
            buffer: &TextBuffer,
            builder: &Builder,
        ) -> Result<()> {
            let main_window: ApplicationWindow = builder.object("main_window").unwrap();
                // log::debug!("{}", &data);

            let (start, end) = buffer.bounds();
            let data = buffer.text(&start, &end, true).map(|v| v.to_string());

            match data {
                Some(data) => {
                    // log::debug!("{}", &data);

                    if let Err(e) = dbus_client::write_file(&path.as_ref(), &data) {
                        log::error!("{}", e);

                        let message = "Could not write file".to_string();
                        let secondary =
                            format!("Error writing to file {}: {}", &path.as_ref().display(), e);

                        let message_dialog = MessageDialogBuilder::new()
                            .parent(&main_window)
                            .destroy_with_parent(true)
                            .decorated(true)
                            .message_type(MessageType::Error)
                            .text(&message)
                            .secondary_text(&secondary)
                            .title("Error")
                            .buttons(ButtonsType::Ok)
                            .build();

                        message_dialog.run();
                        message_dialog.hide();

                        Err(ProfilesError::MethodCallError {
                            description: "Could not write file".to_string(),
                        }
                        .into())
                    } else {
                        log::info!("Wrote file: {}", &path.as_ref().display());

                        Ok(())
                    }
                }

                _ => {
                    log::error!("Could not get buffer contents");

                    Err(ProfilesError::UnknownError {
                        description: "Could not get buffer contents".to_string(),
                    }
                    .into())
                }
            }
        }
    }
}

cfg_if::cfg_if! {
    if #[cfg(feature = "sourceview")] {
        /// Instantiate one page per .profile or .lua file, each page holds a GtkSourceView widget
        /// showing the respective files contents
        fn populate_stack_widget<P: AsRef<Path>>(builder: &Builder, profile: P) -> Result<()> {
            let stack_widget: Stack = builder.object("profile_stack").unwrap();
            let stack_switcher: StackSwitcher = builder.object("profile_stack_switcher").unwrap();

            let context = stack_switcher.style_context();
            context.add_class("small-font");

            let language_manager = sourceview4::LanguageManager::default().unwrap();

            let toml = language_manager.language("toml").unwrap();
            let lua = language_manager.language("lua").unwrap();

            // load and show .profile file
            let source_code = std::fs::read_to_string(&PathBuf::from(&profile.as_ref())).unwrap();

            let mut buffer_index = 0;
            let buffer = BufferBuilder::new()
                .language(&toml)
                .highlight_syntax(true)
                .text(&source_code)
                .build();

            // add buffer to global text buffers map for later reference
            TEXT_BUFFERS.with(|b| {
                let mut text_buffers = b.borrow_mut();
                text_buffers.insert(
                    PathBuf::from(&profile.as_ref()),
                    (buffer_index, buffer.clone()),
                );
            });

            buffer_index += 1;

            let sourceview = sourceview4::View::with_buffer(&buffer);
            sourceview.set_show_line_marks(true);
            sourceview.set_show_line_numbers(true);

            let sourceview = sourceview.dynamic_cast::<TextView>().unwrap();

            sourceview.set_editable(true);

            let filename = profile
                .as_ref()
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string();

            let scrolled_window = ScrolledWindowBuilder::new()
                .shadow_type(ShadowType::None)
                .build();
            scrolled_window.add(&sourceview);

            scrolled_window.show_all();

            stack_widget.add_titled(
                &scrolled_window,
                &profile.as_ref().to_string_lossy(),
                &filename,
            );

            scrolled_window.show_all();

            // add associated .lua files

            for p in util::enumerate_profiles()? {
                if p.profile_file == profile.as_ref() {
                    for f in &p.active_scripts {
                        let abs_path = util::match_script_file(f)?;

                        let source_code = std::fs::read_to_string(&abs_path)?;

                        let buffer = BufferBuilder::new()
                            .language(&lua)
                            .highlight_syntax(true)
                            .text(&source_code)
                            .build();

                        // add buffer to global text buffers map for later reference
                        TEXT_BUFFERS.with(|b| {
                            let mut text_buffers = b.borrow_mut();
                            text_buffers.insert(abs_path.clone(), (buffer_index, buffer.clone()));
                        });

                        buffer_index += 1;

                        // script file editor
                        let sourceview = sourceview4::View::with_buffer(&buffer);
                        sourceview.set_show_line_marks(true);
                        sourceview.set_show_line_numbers(true);

                        let sourceview = sourceview.dynamic_cast::<TextView>().unwrap();

                        sourceview.set_editable(true);

                        let path = f.file_name().unwrap().to_string_lossy().to_string();

                        let scrolled_window = ScrolledWindowBuilder::new().build();
                        scrolled_window.add(&sourceview);

                        stack_widget.add_titled(
                            &scrolled_window,
                            &path,
                            &f.file_name().unwrap().to_string_lossy(),
                        );

                        scrolled_window.show_all();

                        let manifest_file =
                            format!("{}.manifest", abs_path.into_os_string().into_string().unwrap());
                        let f = PathBuf::from(manifest_file);

                        let manifest_data = std::fs::read_to_string(&f)?;

                        let buffer = BufferBuilder::new()
                            .language(&toml)
                            .highlight_syntax(true)
                            .text(&manifest_data)
                            .build();

                        // add buffer to global text buffers map for later reference
                        TEXT_BUFFERS.with(|b| {
                            let mut text_buffers = b.borrow_mut();
                            text_buffers.insert(f.clone(), (buffer_index, buffer.clone()));
                        });

                        buffer_index += 1;

                        // manifest file editor
                        let sourceview = sourceview4::View::with_buffer(&buffer);
                        sourceview.set_show_line_marks(true);
                        sourceview.set_show_line_numbers(true);

                        let sourceview = sourceview.dynamic_cast::<TextView>().unwrap();

                        sourceview.set_editable(true);

                        let path = f.file_name().unwrap().to_string_lossy().to_string();

                        let scrolled_window = ScrolledWindowBuilder::new().build();
                        scrolled_window.add(&sourceview);

                        stack_widget.add_titled(
                            &scrolled_window,
                            &path,
                            &f.file_name().unwrap().to_string_lossy(),
                        );

                        scrolled_window.show_all();
                    }

                    break;
                }
            }

            Ok(())
        }
    } else {
        /// Instantiate one page per .profile or .lua file, each page holds a GtkSourceView widget
        /// showing the respective files contents
        fn populate_stack_widget<P: AsRef<Path>>(builder: &Builder, profile: P) -> Result<()> {
            let stack_widget: Stack = builder.object("profile_stack").unwrap();
            let stack_switcher: StackSwitcher = builder.object("profile_stack_switcher").unwrap();

            let context = stack_switcher.style_context();
            context.add_class("small-font");

            // load and show .profile file
            let source_code = std::fs::read_to_string(&PathBuf::from(&profile.as_ref())).unwrap();

            let buffer = TextBufferBuilder::new().text(&source_code).build();

            let text_view = TextViewBuilder::new()
                .buffer(&buffer)
                .build();

            let mut buffer_index = 0;
            // add buffer to global text buffers map for later reference
            TEXT_BUFFERS.with(|b| {
                let mut text_buffers = b.borrow_mut();
                text_buffers.insert(
                    PathBuf::from(&profile.as_ref()),
                    (buffer_index, buffer.clone()),
                );
            });

            buffer_index += 1;

            text_view.set_editable(true);

            let filename = profile
                .as_ref()
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string();

            let scrolled_window = ScrolledWindowBuilder::new()
                .shadow_type(ShadowType::None)
                .build();
            scrolled_window.add(&text_view);

            scrolled_window.show_all();

            stack_widget.add_titled(
                &scrolled_window,
                &profile.as_ref().to_string_lossy(),
                &filename,
            );

            scrolled_window.show_all();

            // add associated .lua files

            for p in util::enumerate_profiles()? {
                if p.profile_file == profile.as_ref() {
                    for f in p.active_scripts {
                        let abs_path = util::match_script_file(&f)?;

                        let source_code = std::fs::read_to_string(&abs_path)?;

                        let buffer = TextBufferBuilder::new()
                            .text(&source_code)
                            .build();

                        // add buffer to global text buffers map for later reference
                        TEXT_BUFFERS.with(|b| {
                            let mut text_buffers = b.borrow_mut();
                            text_buffers.insert(abs_path.clone(), (buffer_index, buffer.clone()));
                        });

                        buffer_index += 1;

                        // script file editor
                        let text_view = TextViewBuilder::new()
                            .buffer(&buffer)
                            .build();

                        text_view.set_editable(true);

                        let path = f.file_name().unwrap().to_string_lossy().to_string();

                        let scrolled_window = ScrolledWindowBuilder::new().build();
                        scrolled_window.add(&text_view);

                        stack_widget.add_titled(
                            &scrolled_window,
                            &path,
                            &f.file_name().unwrap().to_string_lossy(),
                        );

                        scrolled_window.show_all();

                        let manifest_file =
                            format!("{}.manifest", abs_path.into_os_string().into_string().unwrap());
                        let f = PathBuf::from(manifest_file);

                        let manifest_data = std::fs::read_to_string(&f)?;

                        // add buffer to global text buffers map for later reference
                        TEXT_BUFFERS.with(|b| {
                            let mut text_buffers = b.borrow_mut();
                            text_buffers.insert(f.clone(), (buffer_index, buffer.clone()));
                        });

                        buffer_index += 1;

                        // manifest file editor
                        let buffer = TextBufferBuilder::new()
                            .text(&manifest_data)
                            .build();

                        let text_view = TextViewBuilder::new()
                            .buffer(&buffer)
                            .build();

                        text_view.set_editable(true);

                        let path = f.file_name().unwrap().to_string_lossy().to_string();

                        let scrolled_window = ScrolledWindowBuilder::new().build();
                        scrolled_window.add(&text_view);

                        stack_widget.add_titled(
                            &scrolled_window,
                            &path,
                            &f.file_name().unwrap().to_string_lossy(),
                        );

                        scrolled_window.show_all();
                    }

                    break;
                }
            }

            Ok(())
        }
    }
}

/// Initialize page "Profiles"
pub fn initialize_profiles_page<A: IsA<gtk::Application>>(
    application: &A,
    builder: &Builder,
) -> Result<()> {
    let profiles_treeview: TreeView = builder.object("profiles_treeview").unwrap();
    // let sourceview: sourceview4::View = builder.object("source_view").unwrap();

    // profiles list
    let profiles_treestore = TreeStore::new(&[
        glib::Type::U64,
        String::static_type(),
        String::static_type(),
        String::static_type(),
    ]);

    for (index, ref profile) in util::enumerate_profiles()
        .unwrap_or_else(|_| vec![])
        .iter()
        .enumerate()
    {
        let name = &profile.name;
        let filename = profile
            .profile_file
            .file_name()
            .unwrap_or_else(|| OsStr::new("<error>"))
            .to_string_lossy()
            .to_owned()
            .to_string();

        let path = profile
            .profile_file
            // .file_name()
            // .unwrap_or_else(|| OsStr::new("<error>"))
            .to_string_lossy()
            .to_owned()
            .to_string();

        profiles_treestore.insert_with_values(
            None,
            None,
            &[(0, &(index as u64)), (1, &name), (2, &filename), (3, &path)],
        );
    }

    let id_column = TreeViewColumnBuilder::new()
        .title(&"ID")
        .sizing(TreeViewColumnSizing::Autosize)
        .visible(false)
        .build();
    let name_column = TreeViewColumnBuilder::new()
        .title(&"Name")
        .sizing(TreeViewColumnSizing::Autosize)
        .build();
    let filename_column = TreeViewColumnBuilder::new()
        .title(&"Filename")
        .sizing(TreeViewColumnSizing::Autosize)
        .build();
    let path_column = TreeViewColumnBuilder::new()
        .visible(false)
        .title(&"Path")
        .build();

    let cell_renderer_id = CellRendererText::new();
    let cell_renderer_name = CellRendererText::new();
    let cell_renderer_filename = CellRendererText::new();

    id_column.pack_start(&cell_renderer_id, false);
    name_column.pack_start(&cell_renderer_name, true);
    filename_column.pack_start(&cell_renderer_filename, true);

    profiles_treeview.insert_column(&id_column, 0);
    profiles_treeview.insert_column(&name_column, 1);
    profiles_treeview.insert_column(&filename_column, 2);
    profiles_treeview.insert_column(&path_column, 3);

    id_column.add_attribute(&cell_renderer_id, &"text", 0);
    name_column.add_attribute(&cell_renderer_name, &"text", 1);
    filename_column.add_attribute(&cell_renderer_filename, &"text", 2);
    path_column.add_attribute(&cell_renderer_filename, &"text", 3);

    profiles_treeview.set_model(Some(&profiles_treestore));

    profiles_treeview.connect_row_activated(clone!(@weak builder => move |tv, path, _column| {
        let profile = tv.model().unwrap().value(&tv.model().unwrap().iter(&path).unwrap(), 3).get::<String>().unwrap();

        let _result = populate_visual_config_editor(&builder, &profile).map_err(|e| { log::error!("{}", e) });

        remove_elements_from_stack_widget(&builder);
        let _result = populate_stack_widget(&builder, &profile).map_err(|e| { log::error!("{}", e) });
    }));

    profiles_treeview.show_all();

    update_profile_state(&builder)?;
    register_actions(application, &builder)?;

    Ok(())
}

/// Register global actions and keyboard accelerators
fn register_actions<A: IsA<gtk::Application>>(application: &A, builder: &Builder) -> Result<()> {
    let application = application.as_ref();

    let stack_widget: Stack = builder.object("profile_stack").unwrap();
    // let stack_switcher: StackSwitcher = builder.object("profile_stack_switcher").unwrap();

    let save_current_buffer = gio::SimpleAction::new("save-current-buffer", None);
    save_current_buffer.connect_activate(clone!(@weak builder => move |_, _| {
        if let Some(view) = stack_widget.visible_child()
        // .map(|w| w.dynamic_cast::<sourceview4::View>().unwrap())
        {
            let index = stack_widget.child_position(&view) as usize;

            TEXT_BUFFERS.with(|b| {
                if let Some((path, buffer)) = b
                    .borrow()
                    .iter()
                    .find(|v| v.1 .0 == index)
                    .map(|v| (v.0, &v.1 .1))
                {
                    let _result = save_buffer_contents_to_file(&path, &buffer, &builder);
                }
            });
        }
    }));

    application.add_action(&save_current_buffer);
    application.set_accels_for_action("app.save-current-buffer", &["<Primary>S"]);

    let save_all_buffers = gio::SimpleAction::new("save-all-buffers", None);
    save_all_buffers.connect_activate(clone!(@weak builder => move |_, _| {
        TEXT_BUFFERS.with(|b| {
            'SAVE_LOOP: for (k, (_, v)) in b.borrow().iter() {
                let result = save_buffer_contents_to_file(&k, &v, &builder);

                // stop saving files if an error occurred, or auth has failed
                if result.is_err() {
                    break 'SAVE_LOOP;
                }
            }
        });
    }));

    application.add_action(&save_all_buffers);
    application.set_accels_for_action("app.save-all-buffers", &["<Primary><Shift>S"]);

    Ok(())
}

pub fn update_profile_state(builder: &Builder) -> Result<()> {
    let profiles_treeview: TreeView = builder.object("profiles_treeview").unwrap();

    let model = profiles_treeview.model().unwrap();

    let state = crate::STATE.read();
    let active_profile = state
        .active_profile
        .clone()
        .unwrap_or_else(|| "".to_string());

    model.foreach(|model, path, iter| {
        let item = model.value(iter, 3).get::<String>().unwrap();
        if item == active_profile {
            // found a match
            profiles_treeview.selection().select_iter(&iter);
            profiles_treeview.row_activated(&path, &profiles_treeview.column(1).unwrap());

            true
        } else {
            false
        }
    });

    Ok(())
}
