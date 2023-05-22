use std::collections::LinkedList;
use std::fmt::Debug;
use std::fmt::Display;

use crate::config::TransitionAction;
use crate::config::TransitionTrigger;
use crate::config::TransitionTriggerSequence;
use crate::config::{Property};
use rs_graph::classes;
use rs_graph::{Buildable, Builder};
use rs_graph::linkedlistgraph::*;
use rs_graph::traits::*;
use rs_graph::{LinkedListGraph, VecGraph};
use rs_graph_derive::Graph;
use serde::Deserialize;
use anyhow::Result;
use titlecase::titlecase;

#[derive(Clone, Default, Debug)]
pub struct EdgeData {
    /// Actions that cause this transition
    /// detected by the state machine and actually
    /// cause the state machine to change
    pub actions: Vec<TransitionAction>,
    /// triggers that can be used to cause this transition
    /// e.g. a button press
    /// should result in the associated action occuring
    pub triggers: Vec<TransitionTrigger>,
    ids: Vec<Edge<usize>>,
}

#[derive(Debug, PartialEq, Deserialize, Clone)]
pub struct Transition {
    pub to: String,
    #[serde(default)]
    pub from: Vec<String>,
    #[serde(default)]
    pub actions: Vec<TransitionAction>,
    #[serde(default)]
    pub triggers: Vec<TransitionTrigger>,
    #[serde(skip)]
    ids: Vec<Edge<usize>>,
}

#[derive(Deserialize, Default, Clone, Debug)]
#[serde(rename_all(deserialize = "kebab-case"))]
pub struct State {
    pub name: String,
    #[serde(default)]
    properties: Vec<Property>,
    #[serde(skip)]
    node: Option<Node<usize>>,
}

#[derive(Graph)]
pub struct StateGraph {
    #[graph]
    graph: LinkedListGraph<usize, State, EdgeData>,
    #[nodeattrs(State)]
    states: Vec<State>,
    #[edgeattrs(EdgeData)]
    edges: Vec<EdgeData>,
}

impl Debug for StateGraph {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StateGraph")
            .field("states", &self.states)
            .field("edges", &self.edges)
            .finish()
    }
}

impl From<Transition> for EdgeData {
    fn from(t: Transition) -> Self {
        Self {
            actions: t.actions,
            triggers: t.triggers,
            ids: t.ids,
        }
    }
}

impl Display for TransitionTriggerSequence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "\t* {} {} ", self.action, titlecase(&self.control.replace("_", " ")))?;
        if let Some(duration) = self.duration {
            write!(f, "for {}ms", duration)?;
        }
        Ok(())
    }
}

impl Display for TransitionTrigger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)?;
        if let Some(desc) = &self.description {
            write!(f, ": {}", desc.to_lowercase())?;
        }
        if let Some(timeout) = self.timeout {
            write!(f, "timeout: {}", timeout)?;
        }
        if !self.sequence.is_empty() {
            write!(f, " from {}", self.from.join(", "))?;
            if self.from.len() == 0 {
                write!(f, "any state")?;
            }
            write!(f, "\n")?;
            let mut has_duration = false;
            for seq in self.sequence.iter() {
                if seq.duration.is_some() {
                    has_duration = true;
                }
                write!(f, "{}\n", seq)?;
            }
            if !has_duration {
                write!(f, "\t* Until device enters state {}\n", self.to)?;
            }
        }
        Ok(())
    }
}

pub struct StateMachine {
    states: StateGraph,
}

impl StateMachine {
    pub fn new(mut states: Vec<State>, mut transitions: Vec<Transition>) -> Result<Self> {
        let mut g: LinkedListGraphBuilder<usize, State, EdgeData> = LinkedListGraph::new_builder();
        
        for trans in transitions.iter_mut() {
            let (to, from) = get_states_for_transition(&states, &trans)?;

            log::trace!("Adding transition to {} from {:?}", to, from);
            let to_node = match states[to].node {
                Some(node) => {
                    log::trace!("State {} already has node {}", states[to].name, node);
                    node
                }
                None => {
                    states[to].node = Some(g.add_node());
                    states[to].node.unwrap()
                }
            };

            for from in from {
                let from_node = match states[from].node {
                    Some(node) => {
                        log::trace!("State {} already has node {}", states[to].name, node);
                        node
                    }
                    None => {
                        states[from].node = Some(g.add_node());
                        states[from].node.unwrap()
                    }
                };
                let edge = g.add_edge(to_node, from_node);
                trans.ids.push(edge);
                log::trace!("Added edge {} from {} to {}", g.edge2id(edge), states[from].name, states[to].name);
            }
        }

        let sg: StateGraph = StateGraph {
            graph: g.into_graph(),
            states,
            edges: transitions.into_iter().map(|t| t.into()).collect(),
        };

        //log::info!("State graph: {:#?}", sg);

        Ok(Self { states: sg })
    }

    pub fn list_triggers(&self) -> impl Iterator<Item = &TransitionTrigger> {
        self.states
            .edges
            .iter()
            .flat_map(|e| e.triggers.iter().filter(|t| t.sequence.len() != 0))
    }
}

fn get_states_for_transition(states: &Vec<State>, ts: &Transition) -> Result<(usize, Vec<usize>)> {
    let from = ts
        .from
        .iter()
        .map(|f| {
            states
                .iter()
                .position(|s| s.name == *f)
                .ok_or_else(|| anyhow::anyhow!("State {} not found", f))
        })
        .collect::<Result<Vec<_>>>()?;

    let to = states
        .iter()
        .position(|s| s.name == ts.to)
        .ok_or_else(|| anyhow::anyhow!("State {} not found", ts.to))?;

    let from = if from.len() == 0 {
        states.iter().enumerate().filter_map(|e| if e.0 == to {
            None
        } else {
            Some(e.0)
        }).collect()
    } else {
        from
    };

    Ok((to, from))
}
