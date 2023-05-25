#![feature(async_fn_in_trait)]
#![feature(async_closure)]

#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate log;

pub mod config;
pub mod connections;
pub mod state;
pub mod controls;

use config::{Device, Property};
pub use connections::ConnectionEvent;

use anyhow::Result;
use connections::{Connections, Connection, SerialAction, Connectable};
use futures::channel::mpsc::unbounded;
use state::StateMachine;
use tokio::sync::{mpsc::unbounded_channel, watch, broadcast::{channel, Sender, Receiver}};

#[derive(Clone, Debug)]
pub struct ConnectionEventData {
    pub device: String,
    pub event: ConnectionEvent,
}

#[derive(Clone, Debug)]
pub enum Event {
    //ApplyProperties(Vec<Property>),
    ConnectionEvent(ConnectionEventData),
}

async fn conn_event(ev: ConnectionEventData, sm: &mut StateMachine, ptx: &Sender<Vec<Property>>) {
    let log_target = format!("device:{}", ev.device);
    match ev.event {
        ConnectionEvent::NewLine(line) => {
            if let Some(props) = sm.process_line(&line) {
                let _ = ptx.send(props).map_err(|e| error!("{}", e));
            }
            log::info!(target: &log_target, "{}", line);
        }
        ConnectionEvent::Bytes(bytes) => {
            log::trace!(target: &log_target, "{:?}", bytes);
        }
    }
}

async fn process_event(ev: Event, sm: &mut StateMachine, ptx: &Sender<Vec<Property>>) {
    match ev {
        Event::ConnectionEvent(ev) => conn_event(ev, sm, ptx).await,
    };
}

pub async fn main_loop(device: Device) -> Result<()> {
    let (tx, mut rx) = unbounded_channel::<Event>();
    let (ptx, prx) = channel::<Vec<Property>>(8);
    let mut connections = Connections::new(tx.clone(), prx, &device.connections).await?;
    if let Some(Connectable::Serial(s)) = connections.get(connections::ConnectionType::Serial) {
        s.action(SerialAction::Dtr(false)).await?;
        s.action(SerialAction::Rts(false)).await?;
        debug!("DTR/RTS lowered");
    }

    let mut sm = StateMachine::new(device.states.clone(), device.transitions.clone())?;
    let triggers = sm.list_triggers();

    for trigger in triggers {
        log::debug!("{}", trigger);
    }

    let conn_thread = connections.poll();

    let event_thread = tokio::spawn(async move {
        loop {
            let event = rx.recv().await.unwrap();
            //log::trace!("{:?}", &event);
            process_event(event, &mut sm, &ptx).await;
        }
    });

    let _ = tokio::join!(conn_thread, event_thread);

    Ok(())
}