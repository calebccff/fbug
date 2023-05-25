use crate::{config::SerialConfig, ConnectionEventData, Event};
use anyhow::Result;
use as_any::Downcast;
use bytes::{BufMut, BytesMut};
use futures::SinkExt;
use realpath::realpath;
use serialport::{SerialPort, TTYPort};
use std::{borrow::{Cow, BorrowMut}, path::PathBuf, time::Duration, sync::{Mutex, Arc}, ops::Deref};
use std::any::Any;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt}, sync::mpsc::UnboundedSender
};
use tokio_serial::{SerialPortBuilderExt, SerialStream};
use tokio_stream::{StreamExt, Timeout};
use tokio_util::codec::{Decoder, Framed, LinesCodec};

use super::{Connection, ConnectionError, ConnectionEvent};

pub struct Serial {
    tx: UnboundedSender<Event>,
    lines: Framed<SerialStream, LinesCodec>,
    //buf: BytesMut,
    info: SerialConfig,
    ctrl: SerialControl,
}

#[derive(Clone)]
pub struct SerialControl {
    port: Arc<Mutex<TTYPort>>,
}

impl SerialControl {
    pub fn action(&self, action: SerialAction) -> Result<()> {
        let mut port = self.port.lock().unwrap();
        match action {
            SerialAction::Dtr(state) => port.write_data_terminal_ready(state)?,
            SerialAction::Rts(state) => port.write_request_to_send(state)?,
            SerialAction::Baud(baud) => port.set_baud_rate(baud)?,
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub enum SerialAction {
    Dtr(bool),
    Rts(bool),
    Baud(u32),
}

impl Serial {
    async fn open(path: &PathBuf, baud: u32) -> Result<SerialStream> {
        let path = realpath(path)?;
        trace!("Opening serial port {:?} at {} baud", path, baud);
        tokio_serial::new(path.to_string_lossy(), baud)
            .open_native_async()
            .map_err(|e| anyhow!("Failed to open serial port: {}", e))
            .and_then(|mut port| {
                port.set_exclusive(false)
                    .map_err(|e| anyhow!("Failed to set serial port exclusive: {}", e))?;
                Ok(port)
            })
    }

    fn open_raw(path: &PathBuf, baud: u32) -> Result<TTYPort> {
        serialport::new(path.to_string_lossy(), baud)
            .open_native()
            .map_err(|e| anyhow!("Failed to open serial port: {}", e))
    }

    pub fn ctrl(&self) -> SerialControl {
        self.ctrl.clone()
    }

    // pub async fn reopen(&mut self) -> Result<()> {
    //     self.lines.get_mut().deref() = Self::open(&self.info.path, self.info.baud)
    //         .await
    //         .map_err(|_| ConnectionError::OpenFailed)?;
    //     Ok(())
    // }
}

impl Connection for Serial {
    type Info = SerialConfig;
    type Action = SerialAction;

    async fn new(tx: UnboundedSender<Event>, info: &SerialConfig) -> Result<Self, ConnectionError> {
        let port = Self::open(&info.path, info.baud)
            .await
            .map_err(|_| ConnectionError::OpenFailed)?;
        let ctrl = SerialControl { port: Arc::new(Mutex::new(Self::open_raw(&info.path, info.baud).unwrap())) };
        let framed = Framed::with_capacity(port, LinesCodec::new(), 1024);
        Ok(Self {
            tx,
            info: info.clone(),
            lines: framed,
            ctrl,
            //buf: BytesMut::with_capacity(256),
        })
    }

    async fn action(&self, action: Self::Action) -> Result<()> {
        trace!("Serial: {:?}", action);
        self.ctrl.action(action)
        .map_err(|e| anyhow!("Failed to set serial action: {}", e))
    }

    async fn send(&mut self, buf: &str) -> Result<()> {
        self.lines
            .send(buf)
            .await
            .map_err(|e| anyhow!("Failed to write to serial port: {}", e))
    }

    async fn read(&mut self) {
        let run_until = tokio::time::Instant::now() + Duration::from_millis(100);
        while let Ok(line) = self.lines.try_next().await {
            match line {
                Some(line) => {
                    self.tx
                        .send(Event::ConnectionEvent(ConnectionEventData {
                            device: "device:axolotl".to_string(),
                            event: ConnectionEvent::NewLine(line),
                        }))
                        .unwrap();
                    // Timeout and return so that actions can be handled
                    if run_until > tokio::time::Instant::now() {
                        break;
                    }
                }
                None => {
                    // error!("Error reading from serial port");
                    // break;
                }
            }
        }
    }

    fn name(&self) -> &str {
        &self.info.label
    }
}
