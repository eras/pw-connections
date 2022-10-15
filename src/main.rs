use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time;

use libspa as spa;
use pipewire as pw;
use spa::ReadableDict;

// fn info_callback(info: &pw::Info) {
//     println!("info: {info:?}");
// }

// fn done_callback(a: u32, b: spa::AsyncSeq) {
//     println!("done: {a:?} {b:?}");
// }

// fn error_callback(a: u32, b: i32, c: i32, msg: &str) {
//     println!("error: {a:?} {b:?} {c:?} {msg}");
// }

#[derive(Debug, Clone, Eq, Hash, PartialOrd, PartialEq)]
struct ObjectId(String);

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
struct PortName(String);

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
    link_input_port: ObjectId,
    link_output_node: NodeId,
    link_output_port: ObjectId,
}

#[derive(Debug)]
struct Main {
    ports: HashMap<ObjectId, Port>,              // by port name
    links: HashMap<(NodeId, NodeId), Vec<Link>>, // by source node, by destination node
    links_by_id: HashMap<ObjectId, (NodeId, NodeId)>,
}

impl Main {
    fn new() -> Self {
        Main {
            ports: HashMap::default(),
            links: HashMap::default(),
            links_by_id: HashMap::default(),
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
                    let key = object.id;
                    let port = Port {
                        node_id,
                        port_name,
                        port_id,
                        port_direction,
                    };
                    // dbg!(&port);
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
                    let link_input_port = ObjectId(link_input_port.clone());
                    let link_input_node = NodeId(link_input_node.clone());
                    let link_output_port = ObjectId(link_output_port.clone());
                    let link_output_node = NodeId(link_output_node.clone());

                    let key = (link_input_node.clone(), link_output_node.clone());

                    let e = self.links.entry(key.clone());
                    let link = Link {
                        link_input_node,
                        link_input_port,
                        link_output_node,
                        link_output_port,
                    };

                    e.or_default().push(link);

                    assert!(matches!(self.links_by_id.insert(object.id, key), None));
                } else {
                    //println!("got {object:?}");
                }
            }
            Message::Remove(id) => {
                self.ports.remove(&id);
                if let Some(key) = self.links_by_id.remove(&id) {
                    self.links.remove(&key);
                }
            }
        }
    }

    fn control_thread(&mut self, rx: Receiver<Message>, tx: pw::channel::Sender<PWRequest>) {
        println!("Control thread starting");
        let mut tries = 2;
        let mut stable; // seems things are settled, no messages in a short while
        let mut enable_dump = false;
        loop {
            let message = match rx.recv_timeout(time::Duration::from_secs(1)) {
                Ok(message) => Some(message),
                Err(RecvTimeoutError::Timeout) => None,
                Err(_err) => panic!("message receive failed"),
            };
            if let Some(message) = message {
                if enable_dump {
                    dbg!(&message);
                }
                self.process_message(message);
                stable = false;
            } else {
                stable = true;
            }

            let src_name = "Novation SL MkIII 1:(playback_0) Novation SL MkIII MIDI 1";
            let dst_name = "Virtual Raw MIDI 4-1 4:(capture_0) VirMIDI 4-1";
            let mut has_link = false;

            let mut src_port = None;
            let mut dst_port = None;
            for (_id, port) in self.ports.iter() {
                if port.port_name.0.as_str() == src_name {
                    src_port = Some(port.clone());
                }
                if port.port_name.0.as_str() == dst_name {
                    dst_port = Some(port.clone());
                }
            }

            for ((_src, _dst), links) in self.links.iter() {
                for link in links {
                    let port_in = self.ports.get(&link.link_input_port).expect("wtf");
                    let port_out = self.ports.get(&link.link_output_port.clone()).expect("wtf");
                    let _x = &port_in.port_name.0.as_str();
                    println!("{port_in:?}->{port_out:?}");
                    if port_in.port_name.0.as_str() == src_name
                        && port_out.port_name.0.as_str() == dst_name
                    {
                        has_link = true;
                        // println!("link: {link:?}: {port_in:?} -> {port_out:?}");
                    }
                }
            }
            dbg!(has_link);
            if stable && !has_link {
                enable_dump = true;
                if let (Some(src_port), Some(dst_port)) = (src_port, dst_port) {
                    if tries > 0 {
                        tries -= 1;
                        println!("Sending link request");
                        tx.send(PWRequest::MakeLink((src_port.clone(), dst_port.clone())))
                            .expect("communicating with pw failed");
                    } else {
                        println!("Stopped sending link requests");
                    }
                }
            }
        }
    }
}

fn main() {
    pw::init();

    let mainloop = pw::MainLoop::new().expect("Failed to create Pipewire Mainloop");
    let context = pw::Context::new(&mainloop).expect("Failed to create Pipewire Context");
    let core = context
        .connect(None)
        .expect("Failed to connect to Pipewire Core");

    // // This call uses turbofish syntax to specify that we want a link.
    // let link = core
    //     .create_object::<pw::link::Link, _>(
    //         // The actual name for a link factory might be different for your system,
    //         // you should probably obtain a factory from the registry.
    //         "link-factory",
    //         &pw::properties! {
    //             "link.output.port" => "1",
    //             "link.input.port" => "2",
    //             "link.output.node" => "3",
    //             "link.input.node" => "4"
    //         },
    //     )
    //     .expect("Failed to create object");

    // println!("{link:?}");

    // let _listener = core
    //     .add_listener_local()
    //     .error(error_callback)
    //     .info(info_callback)
    //     .done(done_callback)
    //     .register();

    let (global_tx, global_rx) = channel::<Message>();
    let global_remove_tx = global_tx.clone();

    let (pwcontrol_tx, pwcontrol_rx) = pw::channel::channel();

    let _receiver = pwcontrol_rx.attach(&mainloop, {
        let mainloop = mainloop.clone();
        let core = core.clone();
        let linksies = Rc::new(RefCell::new(Vec::new()));
        move |request| match request {
            PWRequest::Quit => mainloop.quit(),
            PWRequest::MakeLink((input, output)) => {
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
                println!("Link: {link:?}");
                linksies.borrow_mut().push(link);
            }
        }
    });

    let registry = core.get_registry().expect("wtf");
    let _registry_listener = registry
        .add_listener_local()
        .global(move |msg| global_callback(&global_tx, msg))
        .global_remove(move |msg| global_remove_callback(&global_remove_tx, msg))
        .register();

    let mut main = Main::new();
    let _thread = thread::spawn(move || main.control_thread(global_rx, pwcontrol_tx));

    mainloop.run();
}
