use crate::config::{ConnectionInfo, GlobalProperties, Property};
use crate::Event;
use anyhow::Result;
use serial::Serial;
use std::io::ErrorKind;
use std::vec;
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::broadcast::Receiver;
use tokio::sync::mpsc::UnboundedSender;

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
    async fn action(&self, action: Self::Action) -> Result<()>;
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
    prx: Receiver<Vec<Property>>,
}

impl Connections {
    pub async fn new(
        tx: UnboundedSender<Event>,
        prx: Receiver<Vec<Property>>,
        c_info: &Vec<ConnectionInfo>,
    ) -> Result<Self> {
        let mut connections: Vec<Connectable> = vec![];
        let mut c_info = c_info.clone();

        for info in c_info.iter_mut() {
            log::trace!("Connecting to {:?}", info);
            match info {
                ConnectionInfo::Serial(info) => match Serial::new(tx.clone(), info).await {
                    Ok(serial) => connections.push(Connectable::Serial(serial)),
                    Err(e) => {
                        bail!(e);
                    }
                },
                ConnectionInfo::Ssh(_) => {}
                ConnectionInfo::Usb(_) => {}
            }
        }

        let c = Self {
            connections,
            c_info,
            tx,
            prx,
        };

        Ok(c)
    }

    pub fn get(&mut self, c_type: ConnectionType) -> Option<&mut Connectable> {
        self.connections.iter_mut().find(|c| match c {
            Connectable::Serial(_) => c_type == ConnectionType::Serial,
            Connectable::Ssh => c_type == ConnectionType::Ssh,
            Connectable::Usb => c_type == ConnectionType::Usb,
        })
    }

    pub fn find(&mut self, name: &str) -> Option<&mut Connectable> {
        self.connections.iter_mut().find(|c| match c {
            Connectable::Serial(s) => s.name() == name,
            Connectable::Ssh => false,
            Connectable::Usb => false,
        })
    }

    pub async fn poll(mut self) -> Result<()> {
        let ctrl = if let Connectable::Serial(s) = self.get(ConnectionType::Serial).unwrap() {
            s.ctrl()
        } else {
            bail!("No serial connection found");
        };
        let  read_thread = tokio::spawn(async move {
            loop {
                for c in self.connections.iter_mut() {
                    match c {
                        Connectable::Serial(s) => s.read().await,
                        Connectable::Ssh => unimplemented!(),
                        Connectable::Usb => unimplemented!(),
                    }
                };
            }
        });

        let action_thread = tokio::spawn(async move {
            loop {
                if let Ok(props) = self.prx.recv().await {
                    for prop in props {
                        match prop.name {
                            GlobalProperties::Baud(x) => {
                                let _ = ctrl.action(SerialAction::Baud(x)).map_err(|e| {
                                    log::error!("Failed to set baud rate: {}", e);
                                });
                            }
                        }
                    }
                }
            }
        });

        let (r1, r2) = tokio::join!(read_thread, action_thread);
        r1?;
        r2?;

        Ok(())
    }
}
