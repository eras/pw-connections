mod config;
mod error;

use std::cell::RefCell;
use std::collections::HashMap;
use std::convert::From;
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time;

use libspa as spa;
use pipewire as pw;
use spa::ReadableDict;

use clap::Parser;

use config::PortName;

// fn info_callback(info: &pw::Info) {
//     println!("info: {info:?}");
// }

// fn done_callback(a: u32, b: spa::AsyncSeq) {
//     println!("done: {a:?} {b:?}");
// }

// fn error_callback(a: u32, b: i32, c: i32, msg: &str) {
//     println!("error: {a:?} {b:?} {c:?} {msg}");
// }

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Name of the config file to use
    #[arg(short, long)]
    config: String,
}

#[derive(Debug, Clone, Eq, Hash, PartialOrd, PartialEq)]
struct ObjectId(String);

impl<Direction> From<PortObjectId<Direction>> for ObjectId {
    fn from(object_id: PortObjectId<Direction>) -> Self {
        return ObjectId(object_id.0);
    }
}

impl From<LinkObjectId> for ObjectId {
    fn from(object_id: LinkObjectId) -> Self {
        return ObjectId(object_id.0);
    }
}

#[derive(Debug, Clone, Eq, Hash, PartialOrd, PartialEq)]
struct Input();

#[derive(Debug, Clone, Eq, Hash, PartialOrd, PartialEq)]
struct Output();

#[derive(Debug, Clone, Eq, Hash, PartialOrd, PartialEq)]
struct Unknown();

#[derive(Debug, Clone, Eq, Hash, PartialOrd, PartialEq)]
struct PortObjectId<Direction>(String, PhantomData<Direction>);

impl<Direction> From<ObjectId> for PortObjectId<Direction> {
    fn from(object_id: ObjectId) -> Self {
        return PortObjectId(object_id.0, PhantomData);
    }
}

impl PortObjectId<Unknown> {
    fn input(self) -> PortObjectId<Input> {
        return PortObjectId::<Input>(self.0, PhantomData);
    }

    fn output(self) -> PortObjectId<Output> {
        return PortObjectId::<Output>(self.0, PhantomData);
    }
}

impl PortObjectId<Input> {
    fn unknown(self) -> PortObjectId<Unknown> {
        return PortObjectId::<Unknown>(self.0, PhantomData);
    }
}

impl PortObjectId<Output> {
    fn unknown(self) -> PortObjectId<Unknown> {
        return PortObjectId::<Unknown>(self.0, PhantomData);
    }
}

impl<Direction> From<&str> for PortObjectId<Direction> {
    fn from(str: &str) -> Self {
        return PortObjectId(String::from(str), PhantomData);
    }
}

#[derive(Debug, Clone, Eq, Hash, PartialOrd, PartialEq)]
struct LinkObjectId(String);

impl From<ObjectId> for LinkObjectId {
    fn from(object_id: ObjectId) -> Self {
        return LinkObjectId(object_id.0);
    }
}

#[derive(Debug)]
struct Object {
    id: ObjectId,
    properties: HashMap<String, String>,
}

#[derive(Debug)]
enum Message {
    Object(Object),
    Remove(ObjectId),
}

/// Request to PipeWire
#[derive(Debug)]
enum PWRequest {
    MakeLink((Port, Port)),
    Quit,
}

fn global_callback(
    tx: &Sender<Message>,
    global_object: &pw::registry::GlobalObject<spa::ForeignDict>,
) {
    if let Some(props) = global_object.to_owned().props {
        let id = ObjectId(format!("{}", global_object.id));
        //println!("data: {props:?}");
        let properties: HashMap<String, String> = props
            .iter()
            .map(|(a, b)| (String::from(a), String::from(b)))
            .collect();
        tx.send(Message::Object(Object { id, properties }))
            .expect("wtf");
    }
    //tx.send(Message {}).expect("wtf");
}

fn global_remove_callback(tx: &Sender<Message>, id: u32) {
    //println!("global_remove: {value}");
    tx.send(Message::Remove(ObjectId(format!("{}", id))))
        .expect("wtf");
    //tx.send(Message {}).expect("wtf");
}

#[derive(Debug, Clone, Eq, Hash, PartialOrd, PartialEq)]
enum PortDirection {
    In,
    Out,
}

#[derive(Debug, Clone, Eq, Hash, PartialOrd, PartialEq)]
struct NodeId(String);

#[derive(Debug, Clone, Eq, Hash, PartialOrd, PartialEq)]
struct PortId(String);

#[derive(Debug, Clone)]
struct Port {
    node_id: NodeId,
    port_name: PortName,
    port_id: PortId,
    port_direction: PortDirection,
}

impl From<&String> for PortDirection {
    fn from(str: &String) -> PortDirection {
        match str.as_str() {
            "in" => Self::In,
            "out" => Self::Out,
            _ => panic!("wtf"),
        }
    }
}

#[derive(Debug)]
struct Link {
    link_input_node: NodeId,
    link_input_port: PortObjectId<Input>,
    link_output_node: NodeId,
    link_output_port: PortObjectId<Output>,
}

type Ports = HashMap<PortObjectId<Unknown>, Port>;
type Links = HashMap<(PortObjectId<Output>, PortObjectId<Input>), Vec<Link>>;

#[derive(Debug)]
struct Main {
    ports: Ports,
    links: Links,
    links_by_id: HashMap<LinkObjectId, (PortObjectId<Output>, PortObjectId<Input>)>,
    config_links: config::NamedLinks, // desired state
}

impl Main {
    fn new(config_links: config::NamedLinks) -> Self {
        Main {
            ports: HashMap::default(),
            links: HashMap::default(),
            links_by_id: HashMap::default(),
            config_links: config_links,
        }
    }

    fn process_message(&mut self, message: Message) {
        match message {
            Message::Object(object) => {
                let props = &object.properties;
                if let (Some(port_name), Some(node_id), Some(port_id), Some(port_direction)) = (
                    props.get("port.name"),
                    props.get("node.id"),
                    props.get("port.id"),
                    props.get("port.direction"),
                ) {
                    let port_name = PortName(port_name.clone());
                    let port_id = PortId(port_id.clone());
                    let node_id = NodeId(node_id.clone());
                    let port_direction = PortDirection::from(port_direction);
                    //let key = (node_id.clone(), port_id.clone(), port_direction.clone());
                    let key: PortObjectId<Unknown> = object.id.into();
                    let port = Port {
                        node_id,
                        port_name,
                        port_id,
                        port_direction,
                    };
                    // dbg!(&key, &port);
                    // dbg!(&object);
                    assert!(matches!(self.ports.insert(key, port), None));
                } else if let (
                    Some(link_output_port),
                    Some(link_output_node),
                    Some(link_input_port),
                    Some(link_input_node),
                ) = (
                    // got {"factory.id": "20", "object.serial": "6161", "link.output.port": "97", "link.output.node": "36", "link.input.port": "85", "link.input.node": "36"}
                    props.get("link.output.port"),
                    props.get("link.output.node"),
                    props.get("link.input.port"),
                    props.get("link.input.node"),
                ) {
                    let link_input_port = PortObjectId::<Input>::from(link_input_port.as_ref());
                    let link_input_node = NodeId(link_input_node.clone());
                    let link_output_port = PortObjectId::<Output>::from(link_output_port.as_ref());
                    let link_output_node = NodeId(link_output_node.clone());

                    let key: (PortObjectId<Output>, PortObjectId<Input>) =
                        (link_output_port.clone(), link_input_port.clone());

                    let e = self.links.entry(key.clone());
                    let link = Link {
                        link_input_node,
                        link_input_port,
                        link_output_node,
                        link_output_port,
                    };

                    //dbg!(&link);

                    e.or_default().push(link);

                    assert!(matches!(
                        self.links_by_id.insert(object.id.into(), key),
                        None
                    ));
                } else {
                    //println!("got {object:?}");
                }
            }
            Message::Remove(id) => {
                // try to remove objects from both sets
                self.ports.remove(&id.clone().into());
                if let Some(key) = self.links_by_id.remove(&id.into()) {
                    self.links.remove(&key);
                }
            }
        }
    }

    fn control_thread(&mut self, rx: Receiver<Message>, tx: pw::channel::Sender<PWRequest>) {
        println!("Control thread starting");
        let mut stable; // seems things are settled, no messages in a short while
        let mut enable_dump = false;
        loop {
            let message = match rx.recv_timeout(time::Duration::from_millis(100)) {
                Ok(message) => Some(message),
                Err(RecvTimeoutError::Timeout) => None,
                Err(_err) => panic!("message receive failed"),
            };
            if let Some(message) = message {
                if enable_dump {
                    //dbg!(&message);
                }
                self.process_message(message);
                stable = false;
            } else {
                stable = true;
            }

            let mut name_dir_input_port_id: HashMap<PortName, PortObjectId<Input>> = HashMap::new();

            let mut name_dir_output_port_id: HashMap<PortName, PortObjectId<Output>> =
                HashMap::new();

            // TODO: maintain in self.process_message
            // TODO: deal with multiple ports labeled the same
            // dbg!(());
            for (port_id, port) in self.ports.iter() {
                //dbg!(port_id, port);
                match port.port_direction {
                    PortDirection::In => {
                        name_dir_input_port_id
                            .insert(port.port_name.clone(), port_id.clone().input());
                    }
                    PortDirection::Out => {
                        name_dir_output_port_id
                            .insert(port.port_name.clone(), port_id.clone().output());
                    }
                }
            }

            if stable {
                for named_link in self.config_links.0.iter() {
                    self.do_link(
                        &name_dir_input_port_id,
                        &name_dir_output_port_id,
                        &tx,
                        &named_link.src,
                        &named_link.dst,
                    );
                }
            }
        }
    }

    fn do_link(
        &self,
        name_dir_input_port_id: &HashMap<PortName, PortObjectId<Input>>,
        name_dir_output_port_id: &HashMap<PortName, PortObjectId<Output>>,
        tx: &pw::channel::Sender<PWRequest>,
        src_name: &PortName,
        dst_name: &PortName,
    ) {
        let src_port_id = name_dir_output_port_id.get(&src_name);
        let dst_port_id = name_dir_input_port_id.get(&dst_name);

        //dbg!(src_port_id);

        let has_link = if let (Some(src_port_id), Some(dst_port_id)) = (&src_port_id, &dst_port_id)
        {
            let src_port_id = src_port_id.clone().clone().clone();
            let dst_port_id = dst_port_id.clone().clone().clone();
            self.links.get(&(src_port_id, dst_port_id))
        } else {
            None
        };

        match has_link {
            None => {
                // enable_dump = true;
                if let (Some(src_port_id), Some(dst_port_id)) = (src_port_id, dst_port_id) {
                    let src_port = self
                        .ports
                        .get(&src_port_id.clone().unknown())
                        .expect("could not find port by id")
                        .clone();
                    let dst_port = self
                        .ports
                        .get(&dst_port_id.clone().unknown())
                        .expect("could not find port by id")
                        .clone();
                    eprintln!(
                        "link \"{}\" -> \"{}\"",
                        src_port.port_name.0, &dst_port.port_name.0
                    );
                    //println!("link {src_port:?} -> {dst_port:?}",);
                    tx.send(PWRequest::MakeLink((src_port.clone(), dst_port.clone())))
                        .expect("communicating with pw failed");
                } else {
                    eprintln!(
                        "Cannot link \"{}\" -> \"{}\", both ports not found",
                        src_name.0, dst_name.0
                    );
                }
            }
            Some(_link) => {
                //eprintln!("Already linked: {link:?}")
            }
        }
    }
}

fn work() -> Result<(), error::Error> {
    let args = Args::parse();

    let config = config::Config::load(&args.config)?;

    pw::init();

    let mainloop = pw::MainLoop::new().expect("Failed to create Pipewire Mainloop");
    let context = pw::Context::new(&mainloop).expect("Failed to create Pipewire Context");
    let core = context
        .connect(None)
        .expect("Failed to connect to Pipewire Core");

    let (global_tx, global_rx) = channel::<Message>();
    let global_remove_tx = global_tx.clone();

    let (pwcontrol_tx, pwcontrol_rx) = pw::channel::channel();

    let _receiver = pwcontrol_rx.attach(&mainloop, {
        let mainloop = mainloop.clone();
        let core = core.clone();
        let linksies = Rc::new(RefCell::new(Vec::new()));
        move |request| match request {
            PWRequest::Quit => mainloop.quit(),
            PWRequest::MakeLink((output, input)) => {
                let link = core
                    .create_object::<pw::link::Link, _>(
                        // The actual name for a link factory might be different for your system,
                        // you should probably obtain a factory from the registry.
                        "link-factory",
                        &pw::properties! {
                            "link.output.port" => format!("{}", output.port_id.0),
                            "link.output.node" => format!("{}", output.node_id.0),
                            "link.input.port" => format!("{}", input.port_id.0),
                            "link.input.node" => format!("{}", input.node_id.0)
                        },
                    )
                    .expect("Failed to create object");
                //println!("Link: {link:?}");
                linksies.borrow_mut().push(link);
            }
        }
    });

    let registry = core.get_registry().expect("wtf");

    // let _listener = core
    //     .add_listener_local()
    //     .error(error_callback)
    //     .info(info_callback)
    //     .done(done_callback)
    //     .register();

    let _registry_listener = registry
        .add_listener_local()
        .global(move |msg| global_callback(&global_tx, msg))
        .global_remove(move |msg| global_remove_callback(&global_remove_tx, msg))
        .register();

    let mut main = Main::new(config.links);
    let _thread = thread::spawn(move || main.control_thread(global_rx, pwcontrol_tx));

    mainloop.run();

    Ok(())
}

fn main() {
    match work() {
        Ok(()) => (),
        Err(error) => {
            eprintln!("pw-connections: {error}");
        }
    }
}
