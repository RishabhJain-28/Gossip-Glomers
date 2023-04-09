use std::io::{BufRead, StdoutLock, Write};

use anyhow::Context;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Message<Payload> {
    pub src: String,
    pub dest: String,
    pub body: Body<Payload>,
}

#[derive(Serialize, Deserialize)]
pub struct Body<Payload> {
    // #[serde(rename = "type")]
    // ty: String,
    #[serde(rename = "msg_id")]
    pub id: Option<usize>,
    // #[serde(rename = "in_reply_to")]
    pub in_reply_to: Option<usize>,

    #[serde(flatten)]
    pub payload: Payload, // rest: HashMap<String, serde_json::Value>,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum InitPayload {
    Init(Init),
    InitOk,
}

#[derive(Serialize, Deserialize)]
pub struct Init {
    pub node_id: String,
    pub node_ids: Vec<String>,
}
pub trait Node<S, P> {
    fn from_init(init_state: S, init: Init) -> anyhow::Result<Self>
    where
        Self: Sized;
    fn step(&mut self, input: Message<P>, output: &mut StdoutLock) -> anyhow::Result<()>;
}

pub fn main_loop<S, N, P>(init_state: S) -> anyhow::Result<()>
where
    N: Node<S, P>,
    P: DeserializeOwned,
{
    let stdin = std::io::stdin().lock();
    let mut stdin = stdin.lines();
    let mut stdout = std::io::stdout().lock();

    let init_msg: Message<InitPayload> = serde_json::from_str(
        &stdin
            .next()
            .expect("No Init msg received")
            .context("Failed to read init msg from stdiin")?,
    )
    .context("init message could not be deserialized ")?;

    let  InitPayload::Init(init) = init_msg.body.payload else {
        panic!("First msg should be init");
    };

    let reply = Message {
        src: init_msg.dest,
        dest: init_msg.src,
        body: Body {
            id: Some(0),
            in_reply_to: init_msg.body.id,
            payload: InitPayload::InitOk,
        },
    };
    let mut node: N = Node::from_init(init_state, init).context("node initiliazation failed")?;
    serde_json::to_writer(&mut stdout, &reply).context("Serialize response to init")?;
    stdout.write_all(b"\n").context("write trailing new line")?;

    // let inputs = serde_json::Deserializer::from_reader(stdin).into_iter::<Message<P>>();
    for line in stdin {
        let line = line.context("Maelstrom input from STDIN could not be read")?;
        let input: Message<P> = serde_json::from_str(&line)
            .context("Maelstrom input from STDIN could not be deserilized")?;
        node.step(input, &mut stdout)
            .context("Node step function faield")?;
    }

    Ok(())
}
