use std::sync::Arc;
use std::net::TcpStream;
use std::io::{Write, BufReader, BufWriter};

use mqtt3::{self, MqttRead, MqttWrite};
use serde_json::Value;

const ADAFRUIT_IO: &'static str = "io.adafruit.com:1883";

pub struct MQTT {
    writer: BufWriter<TcpStream>,
    reader: BufReader<TcpStream>,
    username: String,
    password: String,
}

impl MQTT {
    pub fn new() -> MQTT {
        let stream = TcpStream::connect(ADAFRUIT_IO).unwrap();
        let reader = BufReader::new(stream.try_clone().unwrap());
        let writer = BufWriter::new(stream.try_clone().unwrap());
        MQTT {
            reader,
            writer,
            username: "username".to_string(),
            password: "ada-io-key".to_string()
        }
    }

    pub fn send_connect(&mut self) -> Result<mqtt3::Packet, mqtt3::Error> {
        let connect = mqtt3::Packet::Connect(Box::new(mqtt3::Connect {
            protocol: mqtt3::Protocol::MQTT(4),
            keep_alive: 30,
            client_id: "rust-mq-example-pub".to_string(),
            clean_session: true,
            last_will: None,
            username: Some(self.username.clone()),
            password: Some(self.password.clone()),
        }));
        println!("{:?}", connect);
        self.writer.write_packet(&connect);
        self.writer.flush();
        self.reader.read_packet()
    }

    pub fn publish_value(&mut self, prop: &str, value: Value) -> Result<mqtt3::Packet, mqtt3::Error> {
		let publish = mqtt3::Packet::Publish(Box::new(mqtt3::Publish {
			dup: false,
			qos: mqtt3::QoS::AtLeastOnce,
			retain: false,
			topic_name: format!("{}/feeds/{}", self.username, prop).to_owned(),
			pid: Some(mqtt3::PacketIdentifier(10)),
			payload: Arc::new(value.to_string().into_bytes())
		}));
		println!("{:?}", publish);
		self.writer.write_packet(&publish);
		self.writer.flush();
		self.reader.read_packet()
    }
}
