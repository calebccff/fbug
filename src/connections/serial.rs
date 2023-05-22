use std::{borrow::Cow, path::PathBuf};
use realpath::realpath;
use crate::{config::SerialConfig, Event};
use anyhow::Result;
use serialport::SerialPort;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::mpsc::UnboundedSender,
};
use tokio_serial::{SerialPortBuilderExt, SerialStream};

use super::{Connection, ConnectionError, ConnectionEvent};

#[derive(Debug)]
pub struct Serial {
    tx: UnboundedSender<Event>,
    stream: SerialStream,
    info: SerialConfig,
}

#[derive(Clone)]
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
    }

    pub async fn reopen(&mut self) -> Result<()> {
        self.stream = Self::open(&self.info.path, self.info.baud).await
            .map_err(|_| ConnectionError::OpenFailed)?;
        Ok(())
    }
}

impl Connection for Serial {
    type Info = SerialConfig;
    type Action = SerialAction;

    async fn new(tx: UnboundedSender<Event>, info: &SerialConfig) -> Result<Self, ConnectionError> {
        let port = Self::open(&info.path, info.baud).await
            .map_err(|_| ConnectionError::OpenFailed)?;
        Ok(Self {
            tx,
            stream: port,
            info: info.clone(),
        })
    }

    async fn action(&mut self, action: Self::Action) -> Result<()> {
        match action {
            SerialAction::Dtr(state) => self.stream.write_data_terminal_ready(state),
            SerialAction::Rts(state) => self.stream.write_request_to_send(state),
            SerialAction::Baud(baud) => self.stream.set_baud_rate(baud),
        }
        .map_err(|e| anyhow!("Failed to set serial action: {}", e))
    }

    async fn send(&mut self, buf: &str) -> Result<()> {
        let sent = self
            .stream
            .write(buf.as_bytes())
            .await
            .map_err(|e| anyhow!("Failed to write to serial port: {}", e))?;

        if buf.len() == sent {
            Ok(())
        } else {
            Err(anyhow!("Failed to write all bytes to serial port"))
        }
    }

    async fn read(&mut self) {
        let mut buf = vec![0; 1024];
        match self.stream.read_buf(&mut buf).await {
            Ok(n) if n > 0 => {
                buf.truncate(n);
                self.tx.send(Event::ConnectionEvent(ConnectionEvent::Data(buf))).unwrap();
            }
            Ok(_) => {}
            Err(e) => {
                log::debug!("Failed to read from serial port: {}", e);
            }
        }
    }

    fn name(&self) -> &str {
        &self.info.label
    }
}
