use anyhow::Context;
use dist_sys_challenges::*;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    io::StdoutLock,
    time::Duration,
};

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
enum Payload {
    Broadcast {
        message: usize,
    },
    BroadcastOk,
    Read,
    ReadOk {
        messages: Vec<usize>,
    },
    Topology {
        topology: HashMap<String, Vec<String>>,
    },
    TopologyOk,
    Gossip {
        seen: HashSet<usize>,
    },
    // GossipOk {
    //     seen: HashSet<usize>,
    // },
}

enum InjectedPayload {
    Gossip,
}

struct BroadcastNode {
    id: usize,
    node: String,
    messages: HashSet<usize>,
    topology: Vec<String>,
    known: HashMap<String, HashSet<usize>>,
    // inject: std::sync::mpsc::Sender<Event<Payload>>,
}
impl Node<(), Payload, InjectedPayload> for BroadcastNode {
    fn from_init(
        _init_state: (),
        init: Init,
        tx: std::sync::mpsc::Sender<Event<Payload, InjectedPayload>>,
    ) -> anyhow::Result<Self>
    where
        Self: Sized,
    {
        let gossip_tx = tx.clone();
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_millis(300));

            if let Err(_) = gossip_tx.send(Event::Inject(InjectedPayload::Gossip)) {
                break;
            };
        });
        Ok(BroadcastNode {
            id: 1,
            messages: HashSet::new(),
            node: init.node_id,
            known: init
                .node_ids
                .into_iter()
                .map(|id| (id, HashSet::new()))
                .collect(),
            topology: Vec::new(),
            // inject: tx,
        })
    }
    fn step(
        &mut self,
        input: Event<Payload, InjectedPayload>,
        output: &mut StdoutLock,
    ) -> anyhow::Result<()> {
        match input {
            Event::EOF => {}
            Event::Inject(payload) => match payload {
                InjectedPayload::Gossip => {
                    for n in &self.topology {
                        let known_to_n = &self.known[n];
                        Message {
                            src: self.node.clone(),
                            dest: n.clone(),

                            body: Body {
                                id: None,
                                in_reply_to: None,
                                payload: Payload::Gossip {
                                    seen: self
                                        .messages
                                        .iter()
                                        .copied()
                                        .filter(|m| !known_to_n.contains(m))
                                        .collect(),
                                },
                            },
                        }
                        .send(output)
                        .with_context(|| format!("gossip to {}", n))?;
                    }
                }
            },
            Event::Message(input) => {
                let mut reply = input.into_reply(Some(&mut self.id));
                match reply.body.payload {
                    Payload::Broadcast { message } => {
                        reply.body.payload = Payload::BroadcastOk;
                        self.messages.insert(message);
                        reply.send(output).context("reply to bradcast")?;
                    }
                    Payload::Read => {
                        reply.body.payload = Payload::ReadOk {
                            messages: self.messages.clone().into_iter().collect(),
                        };

                        reply.send(output).context("reply to read")?;
                    }
                    Payload::Topology { mut topology } => {
                        reply.body.payload = Payload::TopologyOk;
                        self.topology = topology
                            .remove(&self.node)
                            .unwrap_or_else(|| panic!("no topology given for node {}", self.node));
                        reply.send(output).context("reply to topology")?;
                    }
                    Payload::Gossip { seen } => {
                        // reply.body.payload = Payload::Gossip { seen:  };
                        self.messages.extend(seen);
                    }
                    Payload::ReadOk { .. } | Payload::BroadcastOk { .. } | Payload::TopologyOk => {}
                };
            }
        }
        Ok(())
    }
}

fn main() -> anyhow::Result<()> {
    main_loop::<_, BroadcastNode, _, _>(())
}
