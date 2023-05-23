use std::io::ErrorKind;
use std::{vec};
use crate::Event;
use crate::config::{ConnectionInfo};
use anyhow::Result;
use tokio::sync::mpsc::UnboundedSender;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use serial::Serial;
use thiserror::Error;

mod serial;

pub use serial::SerialAction;

#[derive(Error, Debug)]
pub enum ConnectionError {
    #[error("No such device")]
    NoSuchDevice,
    #[error("Failed to open device")]
    OpenFailed,
    #[error("Other error: {0}")]
    Other(String),
}

#[derive(Clone, Debug)]
pub enum ConnectionEvent {
    NewLine(String),
    Bytes(Vec<u8>),
}

pub trait Connection: Sized {
    type Info: Clone + Send + Sync;
    type Action: Clone + Send + Sync;

    async fn new(tx: UnboundedSender<Event>, info: &Self::Info) -> Result<Self, ConnectionError>;
    async fn action(&mut self, action: Self::Action) -> Result<()>;
    async fn send(&mut self, buf: &str) -> Result<()>;
    async fn read(&mut self);

    fn name(&self) -> &str;
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub enum ConnectionType {
    Serial,
    Ssh,
    Usb,
}

pub enum Connectable {
    Serial(Serial),
    Ssh,
    Usb,
}

pub struct Connections {
    connections: Vec<Connectable>,
    c_info: Vec<ConnectionInfo>,
    tx: UnboundedSender<Event>,
}

impl Connections {
    pub async fn new(tx: UnboundedSender<Event>, c_info: &Vec<ConnectionInfo>) -> Result<Self> {
        let mut connections: Vec<Connectable> = vec![];
        let mut c_info = c_info.clone();

        for info in c_info.iter_mut() {
            log::trace!("Connecting to {:?}", info);
            match info {
                ConnectionInfo::Serial(info) => {
                    match Serial::new(tx.clone(), info).await {
                        Ok(serial) => connections.push(Connectable::Serial(serial)),
                        Err(e) => {
                            bail!(e);
                        }
                    }
                },
                ConnectionInfo::Ssh(_) => {},
                ConnectionInfo::Usb(_) => {},
            }
        }

        let c = Self {
            connections,
            c_info,
            tx,
        };

        Ok(c)
    }

    pub fn get(&mut self, c_type: ConnectionType) -> Option<&mut Connectable> {
        self.connections.iter_mut().find(|c| {
            match c {
                Connectable::Serial(_) => c_type == ConnectionType::Serial,
                Connectable::Ssh => c_type == ConnectionType::Ssh,
                Connectable::Usb => c_type == ConnectionType::Usb,
            }
        })
    }

    pub fn find(&mut self, name: &str) -> Option<&mut Connectable> {
        self.connections.iter_mut().find(|c| {
            match c {
                Connectable::Serial(s) => s.name() == name,
                Connectable::Ssh => false,
                Connectable::Usb => false,
            }
        })
    }

    pub async fn poll(&mut self) -> Result<()> {
        for conn in self.connections.iter_mut() {
            match conn {
                Connectable::Serial(s) => s.read().await,
                Connectable::Ssh => unimplemented!(),
                Connectable::Usb => unimplemented!(),
            }
        }

        Ok(())
    }
}
