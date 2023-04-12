use std::io::{BufRead, StdoutLock, Write};

use anyhow::Context;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message<Payload> {
    pub src: String,
    pub dest: String,
    pub body: Body<Payload>,
}

impl<Payload> Message<Payload> {
    pub fn into_reply(self, id: Option<&mut usize>) -> Self {
        Message {
            src: self.dest,
            dest: self.src,
            body: Body {
                id: id.map(|id| {
                    let mid = *id;
                    *id += 1;
                    mid
                }),
                in_reply_to: self.body.id,
                payload: self.body.payload,
            },
        }
    }

    pub fn send(&self, output: &mut impl Write) -> anyhow::Result<()>
    where
        Payload: Serialize,
    {
        serde_json::to_writer(&mut *output, self).context("Serialize response message")?;
        output.write_all(b"\n").context("write trailing new line")?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum InitPayload {
    Init(Init),
    InitOk,
}

#[derive(Debug, Clone)]
pub enum Event<Payload, InjectedPayload = ()> {
    Message(Message<Payload>),
    Inject(InjectedPayload),
    EOF,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Init {
    pub node_id: String,
    pub node_ids: Vec<String>,
}
pub trait Node<S, P, InjectedPayload> {
    fn from_init(
        init_state: S,
        init: Init,
        inject: std::sync::mpsc::Sender<Event<P, InjectedPayload>>,
    ) -> anyhow::Result<Self>
    where
        Self: Sized;
    fn step(
        &mut self,
        input: Event<P, InjectedPayload>,
        output: &mut StdoutLock,
    ) -> anyhow::Result<()>;
}

pub fn main_loop<S, N, P, IP>(init_state: S) -> anyhow::Result<()>
where
    P: DeserializeOwned + Send + 'static,
    N: Node<S, P, IP>,
    IP: Send + 'static,
{
    let (tx, rx) = std::sync::mpsc::channel();

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
    let mut node: N =
        Node::from_init(init_state, init, tx.clone()).context("node initiliazation failed")?;

    let reply = Message {
        src: init_msg.dest,
        dest: init_msg.src,
        body: Body {
            id: Some(0),
            in_reply_to: init_msg.body.id,
            payload: InitPayload::InitOk,
        },
    };
    serde_json::to_writer(&mut stdout, &reply).context("Serialize response to init")?;
    stdout.write_all(b"\n").context("write trailing new line")?;

    drop(stdin);

    let tx = tx.clone();

    let jh = std::thread::spawn(move || {
        let stdin = std::io::stdin().lock();
        // let stdin = stdin.lines();
        for line in stdin.lines() {
            let line = line.context("Maelstrom input from STDIN could not be read")?;
            let input: Message<P> = serde_json::from_str(&line)
                .context("Maelstrom input from STDIN could not be deserilized")?;
            if let Err(_) = tx.send(Event::Message(input)) {
                return Ok::<_, anyhow::Error>(());
            }
            // node.step(, &mut stdout)
            //     .context("Node step function faield")?;
        }
        let _ = tx.send(Event::EOF);
        Ok(())
    });

    for input in rx {
        node.step(input, &mut stdout)
            .context("Node step function faield")?;
    }

    jh.join()
        .expect("stdin thread paniced")
        .context("stdin thread err ")?;

    Ok(())
}
