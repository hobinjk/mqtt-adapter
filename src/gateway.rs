use std;
use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::sync::mpsc::{channel, Sender, Receiver};
use std::thread;
use std::time::Duration;

use nanomsg::{Protocol, Socket};
use serde_json::{self, Value};

const BASE_URL: &'static str = "ipc:///tmp";
const ADAPTER_MANAGER_URL: &'static str = "ipc:///tmp/gateway.addonManager";

#[derive(Serialize)]
#[serde(tag = "messageType", content = "data", rename_all = "camelCase")]
pub enum PluginRegisterMessage {
    #[serde(rename_all = "camelCase")]
    RegisterPlugin {
        plugin_id: String,
    },
}

#[derive(Deserialize)]
#[serde(tag = "messageType", content = "data", rename_all = "camelCase")]
pub enum GatewayRegisterMessage {
    #[serde(rename_all = "camelCase")]
    RegisterPluginReply {
        plugin_id: String,
        ipc_base_addr: String,
    },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "messageType", content = "data", rename_all = "camelCase")]
pub enum GatewayMessage {
    #[serde(rename_all = "camelCase")]
    UnloadPlugin {
        plugin_id: String,
    },
    #[serde(rename_all = "camelCase")]
    UnloadAdapter {
        plugin_id: String,
        adapter_id: String,
    },

    #[serde(rename_all = "camelCase")]
    SetProperty {
        plugin_id: String,
        adapter_id: String,
        device_id: String,
        property: Property,
    },
    #[serde(rename_all = "camelCase")]
    StartPairing {
        plugin_id: String,
        adapter_id: String,
        timeout: f64,
    },
    #[serde(rename_all = "camelCase")]
    CancelPairing {
        plugin_id: String,
        adapter_id: String,
    },
    #[serde(rename_all = "camelCase")]
    RemoveThing {
        plugin_id: String,
        adapter_id: String,
        device_id: String,
    },
    #[serde(rename_all = "camelCase")]
    CancelRemoveThing {
        plugin_id: String,
        adapter_id: String,
        device_id: String,
    },
}

#[derive(Serialize)]
#[serde(tag = "messageType", content = "data", rename_all = "camelCase")]
pub enum PluginMessage {
    #[serde(rename_all = "camelCase")]
    PluginUnloaded {
        plugin_id: String,
    },
    #[serde(rename_all = "camelCase")]
    AdapterUnloaded {
        plugin_id: String,
        adapter_id: String,
    },

    #[serde(rename_all = "camelCase")]
    AddAdapter {
        plugin_id: String,
        adapter_id: String,
        name: String,
    },
    #[serde(rename_all = "camelCase")]
    HandleDeviceAdded {
        plugin_id: String,
        adapter_id: String,
        id: String,
        name: String,
        typ: String,
        properties: HashMap<String, Property>,
        actions: HashMap<String, Action>,
    },
    #[serde(rename_all = "camelCase")]
    HandleDeviceRemoved {
        plugin_id: String,
        adapter_id: String,
        id: String,
    },
    #[serde(rename_all = "camelCase")]
    PropertyChanged {
        plugin_id: String,
        adapter_id: String,
        device_id: String,
        property: Property,
    },
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Property {
    pub name: String,
    pub value: Value,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Action {
    pub name: String,
}

pub struct GatewayBridge {
    id: String,
    msg_sender: Sender<GatewayMessage>,
    msg_receiver: Receiver<PluginMessage>
}

impl GatewayBridge {
    pub fn new(id: &str) -> (GatewayBridge, Sender<PluginMessage>, Receiver<GatewayMessage>) {
        let (gp_sender, gp_receiver) = channel();
        let (pg_sender, pg_receiver) = channel();
        (
            GatewayBridge {
                id: id.to_string(),
                msg_sender: gp_sender,
                msg_receiver: pg_receiver,
            },
            pg_sender,
            gp_receiver
        )
    }

    pub fn run_forever(&mut self) -> Result<(), io::Error> {
        let mut socket = Socket::new(Protocol::Req)?;
        let mut endpoint = socket.connect(ADAPTER_MANAGER_URL)?;
        let req = PluginRegisterMessage::RegisterPlugin {
            plugin_id: self.id.to_string()
        };
        socket.write_all(serde_json::to_string(&req)?.as_bytes())?;
        let mut rep = String::new();
        socket.read_to_string(&mut rep)?;
        endpoint.shutdown()?;
        println!("We got it! {}", rep);
        let msg: GatewayRegisterMessage = serde_json::from_str(&rep)?;
        // open a Req channel to adapterManager
        // send {messageType: 'registerPlugin', data: { pluginId: id }}
        // receives
        // {
        //  messageType: 'registerPluginReply',
        //  data: {
        //    pluginId: 'pluginId-string',
        //    ipcBaseAddr: 'gateway.plugin.xxx',
        //  },
        //}
        // connect to ipcBaseAddr as pair
        // then handle everything

        let ipc_base_addr = match msg {
            GatewayRegisterMessage::RegisterPluginReply {plugin_id, ipc_base_addr} => {
                if plugin_id != self.id {
                    panic!("mismatched plugin id on channel")
                }
                ipc_base_addr
            },
        };

        let mut socket_pair = Socket::new(Protocol::Pair)?;
        let mut endpoint_pair = socket_pair.connect(&format!("{}/{}", BASE_URL, &ipc_base_addr))?;

        let mut buf = Vec::new();

        loop {
            let read_status = socket.nb_read_to_end(&mut buf);
            if read_status.is_ok() {
                match serde_json::from_slice(&buf) {
                    Ok(msg) => {
                        self.msg_sender.send(msg).unwrap();
                    },
                    _ => {
                    }
                }
            }

            if let Ok(msg_to_send) = self.msg_receiver.try_recv() {
                socket_pair.write_all(serde_json::to_string(&msg_to_send)?.as_bytes()).unwrap();
                match msg_to_send {
                    PluginMessage::PluginUnloaded {..} => {
                        println!("run_forever exiting");
                        endpoint_pair.shutdown()?;
                        return Ok(());
                    }
                    _ => {}
                }
            }

            thread::sleep(Duration::from_millis(33));
        }
    }
}

fn to_io_error<E>(err: E) -> io::Error
    where E: Into<Box<std::error::Error+Send+Sync>> {
    io::Error::new(io::ErrorKind::Other, err)
}

pub trait Device {
    fn set_property(&mut self, property: Property) -> Result<Property, io::Error>;

    fn get_name(&self) -> String {
        "Unknown Device".to_string()
    }

    fn get_type(&self) -> String {
        "thing".to_string()
    }

    fn get_actions(&self) -> HashMap<String, Action> {
        HashMap::new()
    }

    fn get_properties(&self) -> HashMap<String, Property> {
        HashMap::new()
    }
}

pub trait Adapter<T:Device> {
    fn get_name(&self) -> String {
        "Unknown Adapter".to_string()
    }
    fn get_devices(&self) -> &HashMap<String, Box<T>>;

    fn start_pairing(&mut self) -> Result<(), io::Error>;

    fn cancel_pairing(&mut self) -> Result<(), io::Error>;

    fn set_property(&mut self, device_id: &str, property: Property) -> Result<Property, io::Error>;
}

pub struct Plugin<D:Device, A:Adapter<D>> {
    id: String,
    adapters: HashMap<String, Box<A>>,
    sender: Sender<PluginMessage>,
    receiver: Receiver<GatewayMessage>,
    _marker: std::marker::PhantomData<D>,
}

impl<D:Device, A:Adapter<D>> Plugin<D, A> {
    pub fn new(id: &str, sender: Sender<PluginMessage>, receiver: Receiver<GatewayMessage>) -> Plugin<D, A> {
        Plugin {
            id: id.to_string(),
            sender: sender,
            receiver: receiver,
            adapters: HashMap::new(),
            _marker: std::marker::PhantomData,
        }
    }

    fn handle_msg(&mut self, msg: GatewayMessage) -> Result<(), io::Error> {
        match msg {
            GatewayMessage::SetProperty {
                plugin_id,
                adapter_id,
                device_id,
                property
            } => {
                if plugin_id != self.id {
                    return Ok(())
                }

                let set_prop = match self.adapters.get_mut(&adapter_id) {
                    Some(adapter) => adapter.set_property(&device_id, property),
                    None => Err(io::Error::new(io::ErrorKind::Other, "Adapter not found"))
                };
                let prop = set_prop?;
                self.sender.send(PluginMessage::PropertyChanged {
                    plugin_id,
                    adapter_id,
                    device_id,
                    property: prop
                }).map_err(to_io_error)
            },
            GatewayMessage::UnloadPlugin {..} => {
                Ok(())
            },
            GatewayMessage::UnloadAdapter {..} => {
                Ok(())
            },
            GatewayMessage::StartPairing {
                plugin_id,
                adapter_id,
                timeout: _,
            } => {
                if plugin_id != self.id {
                    return Ok(())
                }

                match self.adapters.get_mut(&adapter_id) {
                    Some(adapter) => adapter.start_pairing(),
                    None => Err(io::Error::new(io::ErrorKind::Other, "Adapter not found")),
                }
            },
            GatewayMessage::CancelPairing {
                plugin_id,
                adapter_id,
            } => {
                if plugin_id != self.id {
                    return Ok(())
                }

                match self.adapters.get_mut(&adapter_id) {
                    Some(adapter) => adapter.cancel_pairing(),
                    None => Err(io::Error::new(io::ErrorKind::Other, "Adapter not found")),
                }
            },
            GatewayMessage::RemoveThing { .. } => {
                Ok(())
            },
            GatewayMessage::CancelRemoveThing { .. } => {
                Ok(())
            }
        }
    }

    pub fn add_adapter(&mut self, adapter_id: &str, adapter: Box<A>) {
        self.adapters.insert(adapter_id.to_string(), adapter);
    }

    pub fn run_forever(&mut self) -> Result<(), io::Error> {
        for (adapter_id, adapter) in &self.adapters {
            self.sender.send(PluginMessage::AddAdapter {
                plugin_id: self.id.clone(),
                adapter_id: adapter_id.clone(),
                name: adapter.get_name()
            }).map_err(to_io_error)?;
            for (device_id, device) in adapter.get_devices() {
                self.sender.send(PluginMessage::HandleDeviceAdded {
                    plugin_id: self.id.clone(),
                    adapter_id: adapter_id.clone(),
                    id: device_id.clone(),
                    name: device.get_name(),
                    typ: device.get_type(),
                    actions: device.get_actions(),
                    properties: device.get_properties(),
                }).map_err(to_io_error)?;
            }
        }

        loop {
            match self.receiver.try_recv() {
                Ok(msg) => {
                    println!("recv: {:?}", msg);
                    self.handle_msg(msg)?;
                },
                _ => {}
            }
        }
    }
}
