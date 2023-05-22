
use std::ffi::{OsString, OsStr};
use std::io::ErrorKind;
use std::{vec};
use futures::Future;
use futures::Stream;
use crate::Event;
use crate::config::{ConnectionInfo};
use anyhow::Result;
use inotify::{WatchMask};
use futures::{StreamExt};
use tokio::sync::mpsc::UnboundedSender;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use inotify::Inotify;
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
    Data(Vec<u8>),
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
    inotify: Inotify,
    tx: UnboundedSender<Event>,
}

impl Connections {
    pub async fn new(tx: UnboundedSender<Event>, c_info: &Vec<ConnectionInfo>) -> Result<Self> {
        let mut connections: Vec<Connectable> = vec![];
        let mut inotify = Inotify::init()?;
        let mut c_info = c_info.clone();

        while let Some(info) = c_info.iter_mut().next() {
            match info {
                ConnectionInfo::Serial(info) => {
                    match Serial::new(tx.clone(), info).await {
                        Ok(serial) => connections.push(Connectable::Serial(serial)),
                        Err(e) => {
                            bail!(e);
                        }
                    }
                    let mut dir = info.path.parent().unwrap().clone();
                    while !dir.exists() {
                        dir = match dir.parent() {
                            Some(d) => d,
                            None => break,
                        };
                    }
                    trace!("Adding watch for {:?}", dir);
                    inotify.add_watch(dir, WatchMask::CREATE)?;
                },
                ConnectionInfo::Ssh(_) => unimplemented!(),
                ConnectionInfo::Usb(_) => unimplemented!(),
            }
        }

        let c = Self {
            connections,
            c_info,
            inotify,
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

        // FIXME: Async!
        let mut buf = [0; 1024];
        let events = loop {
        match self.inotify.read_events(&mut buf) {
            Ok(events) => break events,
            Err(error) if error.kind() == ErrorKind::WouldBlock => continue,
            _ => panic!("Error while reading events"),
            }
        };

        for event in events {
            if event.mask.contains(inotify::EventMask::CREATE) {
                let path: &OsStr = match event.name {
                    Some(name) => name,
                    None => continue,
                };
                let info = self.c_info.iter().find(|info| {
                    match info {
                        ConnectionInfo::Serial(info) => info.path == path,
                        ConnectionInfo::Ssh(_) => false,
                        ConnectionInfo::Usb(_) => false,
                    }
                });
                match info {
                    Some(ConnectionInfo::Serial(info)) => {
                        let conn = self.connections.iter_mut().find_map(|c| {
                            match c {
                                Connectable::Serial(s) => if s.name() == info.label {
                                    Some(s)
                                } else {
                                    None
                                },
                                Connectable::Ssh => None,
                                Connectable::Usb => None,
                            }
                        }).unwrap();
                        conn.reopen().await?;
                    },
                    _ => {
                        error!("Couldn't find connection info for {}", path.to_str().unwrap());
                    }
                }
            }
        }

        Ok(())
    }
}
