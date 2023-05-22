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

pub use connections::ConnectionEvent;

#[derive(Clone, Debug)]
pub enum Event {
    StateTransition(state::State),
    ConnectionEvent(connections::ConnectionEvent),
}
