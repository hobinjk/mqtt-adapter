use std::sync::Arc;
use std::net::TcpStream;
use std::io::{Write, BufReader, BufWriter};

use mqtt3::{self, MqttRead, MqttWrite};
use serde_json::Value;

pub struct MQTT {
    writer: BufWriter<TcpStream>,
    reader: BufReader<TcpStream>,
    username: String,
    password: String,
}

impl MQTT {
    pub fn new(server: &str, username: &str, password: &str) -> MQTT {
        let stream = TcpStream::connect(server).unwrap();
        let reader = BufReader::new(stream.try_clone().unwrap());
        let writer = BufWriter::new(stream.try_clone().unwrap());
        MQTT {
            reader,
            writer,
            username: username.to_string(),
            password: password.to_string()
        }
    }

    pub fn send_connect(&mut self) -> Result<mqtt3::Packet, mqtt3::Error> {
        let connect = mqtt3::Packet::Connect(Box::new(mqtt3::Connect {
            protocol: mqtt3::Protocol::MQTT(4),
            keep_alive: 65535, // TODO properly fix
            client_id: "rust-mq-example-pub".to_string(),
            clean_session: true,
            last_will: None,
            username: Some(self.username.clone()),
            password: Some(self.password.clone()),
        }));
        self.writer.write_packet(&connect)?;
        self.writer.flush()?;
        self.reader.read_packet()
    }

    pub fn publish_value(&mut self, prop: &str, value: &Value) -> Result<mqtt3::Packet, mqtt3::Error> {
        let publish = mqtt3::Packet::Publish(Box::new(mqtt3::Publish {
                dup: false,
                qos: mqtt3::QoS::AtLeastOnce,
                retain: false,
                topic_name: format!("{}/feeds/{}", self.username, prop).to_owned(),
                pid: Some(mqtt3::PacketIdentifier(10)),
                payload: Arc::new(value.to_string().into_bytes())
        }));
        self.writer.write_packet(&publish)?;
        self.writer.flush()?;
        self.reader.read_packet()
    }

    pub fn publish_action(&mut self, name: &str) -> Result<mqtt3::Packet, mqtt3::Error> {
        let publish = mqtt3::Packet::Publish(Box::new(mqtt3::Publish {
                dup: false,
                qos: mqtt3::QoS::AtLeastOnce,
                retain: false,
                topic_name: format!("{}/feeds/actions", self.username).to_owned(),
                pid: Some(mqtt3::PacketIdentifier(10)),
                payload: Arc::new(name.to_string().into_bytes())
        }));
        self.writer.write_packet(&publish)?;
        self.writer.flush()?;
        self.reader.read_packet()
    }
}
