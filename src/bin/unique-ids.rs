use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::io::{StdoutLock, Write};

use dist_sys_challenges::*;

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
enum Payload {
    Generate,
    GenerateOk {
        #[serde(rename = "id")]
        guid: String,
    },
}

struct UniqueNode {
    id: usize,
    node: String,
}
impl Node<(), Payload, ()> for UniqueNode {
    fn from_init(
        _init_state: (),
        init: Init,
        _tx: std::sync::mpsc::Sender<Event<Payload>>,
    ) -> anyhow::Result<Self>
    where
        Self: Sized,
    {
        Ok(UniqueNode {
            id: 1,
            node: init.node_id,
        })
    }
    fn step(&mut self, input: Event<Payload>, output: &mut StdoutLock) -> anyhow::Result<()> {
        let Event::Message(input) = input else{
            panic!("got inhjected event when there is no event injection ");
        };
        let mut reply = input.into_reply(Some(&mut self.id));
        match reply.body.payload {
            Payload::Generate => {
                let guid = format!("{}-{}", self.node, self.id);
                reply.body.payload = Payload::GenerateOk { guid };
                serde_json::to_writer(&mut *output, &reply)
                    .context("Serialize response to generate")?;
                output.write_all(b"\n").context("write trailing new line")?;
            }
            Payload::GenerateOk { .. } => {}
        };
        Ok(())
    }
}

fn main() -> anyhow::Result<()> {
    main_loop::<_, UniqueNode, _, ()>(())
}
