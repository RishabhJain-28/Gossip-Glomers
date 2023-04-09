use std::io::{StdoutLock, Write};

use anyhow::{bail, Context};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct Message {
    src: String,
    dest: String,
    body: Body,
}

#[derive(Serialize, Deserialize)]
struct Body {
    // #[serde(rename = "type")]
    // ty: String,
    #[serde(rename = "msg_id")]
    id: Option<usize>,
    // #[serde(rename = "in_reply_to")]
    in_reply_to: Option<usize>,

    #[serde(flatten)]
    payload: Payload, // rest: HashMap<String, serde_json::Value>,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
enum Payload {
    Echo {
        echo: String,
    },
    EchoOk {
        echo: String,
    },
    Init {
        node_id: String,
        node_ids: Vec<String>,
    },
    InitOk,
}

struct EchoNode {
    id: usize,
}

impl EchoNode {
    pub fn step(&mut self, input: Message, output: &mut StdoutLock) -> anyhow::Result<()> {
        match input.body.payload {
            Payload::Init { node_id, node_ids } => {
                let reply = Message {
                    src: input.dest,
                    dest: input.src,
                    body: Body {
                        id: Some(self.id),
                        in_reply_to: input.body.id,
                        payload: Payload::InitOk,
                    },
                };

                serde_json::to_writer(&mut *output, &reply)
                    .context("Serialize response to init")?;
                output.write_all(b"\n").context("write trailing new line")?;
                self.id += 1
            }
            Payload::InitOk => bail!("Received Payload::InitOk"),
            Payload::Echo { echo } => {
                let reply = Message {
                    body: Body {
                        id: Some(self.id),
                        in_reply_to: input.body.id,
                        payload: Payload::EchoOk { echo },
                    },
                    src: input.dest,
                    dest: input.src,
                };
                serde_json::to_writer(&mut *output, &reply)
                    .context("Serialize response to init")?;
                output.write_all(b"\n").context("write trailing new line")?;
                self.id += 1;
            }
            Payload::EchoOk { .. } => {}
        };
        Ok(())
    }
}

fn main() -> anyhow::Result<()> {
    let stdin = std::io::stdin().lock();
    let mut stdout = std::io::stdout().lock();
    let inputs = serde_json::Deserializer::from_reader(stdin).into_iter::<Message>();
    // let mut output = serde_json::Serializer::new(stdout);
    let mut state = EchoNode { id: 0 };

    for input in inputs {
        let input = input.context("Maelstrom input from STDIN could not be deserialized")?;
        state
            .step(input, &mut stdout)
            .context("Node step function faield")?;
    }

    Ok(())
}
