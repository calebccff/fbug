use serde::{Deserialize, Serialize};
use std::{fmt, path::PathBuf};
use strum_macros::Display;
use crate::state::{State, Transition};

#[derive(Debug, Deserialize)]
pub struct Device {
    pub name: String,
    pub codename: String,
    pub description: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub resting_state: Option<String>,
    pub connections: Vec<ConnectionInfo>,
    pub controls: Vec<Control>,
    pub states: Vec<State>,
    pub transitions: Vec<Transition>,
}

// Connections

#[derive(Debug, PartialEq, Display, Deserialize, Clone)]
#[serde(tag = "type", rename_all(deserialize = "kebab-case"))]
pub enum ConnectionInfo {
    Serial(SerialConfig),
    Usb(UsbConnection),
    Ssh(SshConnection),
}

fn _default_baud() -> u32 {
    115200
}

fn _default_lines() -> bool {
    true
}

fn _default_uart_label() -> String {
    "UART".to_string()
}

#[derive(Debug, PartialEq, Deserialize, Clone)]
pub struct SerialConfig {
    #[serde(default = "_default_uart_label")]
    pub label: String,
    #[serde(default)]
    pub getty: bool,
    pub path: PathBuf,
    #[serde(default = "_default_baud")]
    pub baud: u32,
    #[serde(default = "_default_lines")]
    pub lines: bool,
}

fn _default_usb_label() -> String {
    "USB".to_string()
}

#[derive(Debug, PartialEq, Deserialize, Clone)]
pub struct UsbConnection {
    #[serde(default = "_default_usb_label")]
    pub label: String,
    pub port: String,
}

fn _default_ssh_label() -> String {
    "SSH".to_string()
}

#[derive(Debug, PartialEq, Deserialize, Clone)]
pub struct SshConnection {
    #[serde(default = "_default_ssh_label")]
    pub label: String,
    pub host: String,
    pub port: u16,
}

// Controls

#[derive(Debug, PartialEq, Deserialize, Clone)]
#[serde(tag = "type", rename_all(deserialize = "kebab-case"))]
pub struct Control {
    pub name: String,
    pub connection: String,
    #[serde(flatten)]
    pub control_type: ControlType,
}

#[derive(Debug, Display, PartialEq, Deserialize, Clone)]
#[serde(tag = "type", rename_all(deserialize = "kebab-case"))]
pub enum ControlType {
    Button(ButtonControl),
    Command(CommandControl),
}

#[derive(Debug, Display, PartialEq, Deserialize, Clone)]
#[serde(rename_all(deserialize = "kebab-case"))]
pub enum Action {
    Dtr,
    Rts,
    Command,
}

#[derive(Debug, PartialEq, Deserialize, Clone)]
#[serde(rename_all(deserialize = "kebab-case"))]
pub struct ButtonControl {
    pub action: String,
}

#[derive(Debug, PartialEq, Deserialize, Clone)]
#[serde(rename_all(deserialize = "kebab-case"))]
pub struct CommandControl {
    pub command_on: String,
    pub command_off: String,
}

// States

#[derive(Debug, Display, PartialEq, Eq, PartialOrd, Ord, Deserialize, Clone, Copy)]
#[serde(rename_all(deserialize = "kebab-case"))]
pub enum GlobalProperties {
    Baud(u32),
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize, Clone, Copy)]
pub struct Property {
    #[serde(flatten)]
    pub name: GlobalProperties,
}

// Transitions

#[derive(Debug, PartialEq, Deserialize, Clone)]
pub struct TransitionAction {
    /// Connection label
    pub source: String,
    pub event: String,
    pub value: String,
}

#[derive(Debug, PartialEq, Deserialize, Clone)]
pub struct TransitionTrigger {
    #[serde(skip)]
    pub to: String,
    #[serde(default)]
    pub from: Vec<String>,
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub sequence: Vec<TransitionTriggerSequence>,
    pub timeout: Option<u32>,
}

#[derive(Debug, Display, PartialEq, Deserialize, Clone)]
#[serde(rename_all(deserialize = "kebab-case"))]
pub enum ControlAction {
    #[serde(alias = "on")]
    Press,
    #[serde(alias = "on")]
    Release,
    Hold,
}

#[derive(Debug, PartialEq, Deserialize, Clone)]
pub struct TransitionTriggerSequence {
    pub control: String,
    pub action: ControlAction,
    pub duration: Option<u32>,
}

fn validate_config(config: &Device) -> anyhow::Result<()> {
    let mut states = config.states.clone();
    states.dedup_by_key(|s| s.name.clone());
    if states.len() != config.states.len() {
        return Err(anyhow!("Duplicate state names found"));
    }
    let mut transitions = config.transitions.clone();
    transitions.dedup_by(|a, b| a.from == b.from && a.to == b.to);
    if transitions.len() != config.transitions.len() {
        return Err(anyhow!("Duplicate transition names found"));
    }
    for control in config.controls.iter() {
        if config.connections.iter().map(|c| {
            match c {
                ConnectionInfo::Serial(s) => &s.label,
                ConnectionInfo::Usb(u) => &u.label,
                ConnectionInfo::Ssh(s) => &s.label,
            }
        }).find(|conn| *conn == &control.connection).is_none() {
            return Err(anyhow!(
                "Control {} references non-existent connection {}",
                control.name,
                control.connection
            ));
        }
    }
    Ok(())
}

pub fn load_config(path: &PathBuf) -> anyhow::Result<Device> {
    let config = std::fs::read_to_string(path)?;
    let mut device: Device = serde_yaml::from_str(&config)?;
    device.transitions.iter_mut().for_each(|trans| {
        trans.triggers.iter_mut().for_each(|trigger| {
            if trigger.from.is_empty() {
                trigger.from = trans.from.clone();
            };
            trigger.to = trans.to.clone();
        })
    });
    validate_config(&device)?;
    log::trace!("{:#?}", device);
    Ok(device)
}
