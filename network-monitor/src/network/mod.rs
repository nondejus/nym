use std::{sync::Arc, time::Duration};

use crypto::asymmetric::encryption::KeyPair;
use directory_client::{presence::mixnodes::MixNodePresence, Client, DirectoryClient};
use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender};
use gateway_client::GatewayClient;
use log::error;
use mixnet_listener::MixnetListener;
use nymsphinx::{
    acknowledgements::AckKey, addressing::clients::Recipient,
    addressing::nodes::NymNodeRoutingAddress, preparer::MessagePreparer, SphinxPacket,
};
use rand::rngs::OsRng;
use tokio::{runtime::Runtime, time};
use topology::NymTopology;

pub(crate) mod good_topology;
pub(crate) mod mixnet_listener;

const DEFAULT_RNG: OsRng = OsRng;

const DEFAULT_AVERAGE_PACKET_DELAY: Duration = Duration::from_millis(200);
const DEFAULT_AVERAGE_ACK_DELAY: Duration = Duration::from_millis(200);

type MixnetReceiver = UnboundedReceiver<Vec<Vec<u8>>>;
pub(crate) type MixnetSender = UnboundedSender<Vec<Vec<u8>>>;
type AckReceiver = UnboundedReceiver<Vec<Vec<u8>>>;
pub(crate) type AckSender = UnboundedSender<Vec<Vec<u8>>>;

pub struct MonitorConfig {
    pub ack_receiver: AckReceiver,
    pub directory_uri: String,
    pub gateway_client: GatewayClient,
    pub good_topology: NymTopology,
    pub self_address: Recipient,
}

pub struct Monitor {
    config: MonitorConfig,
}

impl Monitor {
    pub fn new(config: MonitorConfig) -> Monitor {
        Monitor { config }
    }

    pub fn run(&mut self, mixnet_receiver: MixnetReceiver, client_encryption_keypair: KeyPair) {
        let mut runtime = Runtime::new().unwrap();
        runtime.block_on(async {
            println!(
                "* self address is:  {}",
                self.config.self_address.to_string()
            );

            self.config
                .gateway_client
                .authenticate_and_start()
                .await
                .expect("Couldn't authenticate with gateway node.");
            println!("* authenticated to gateway");

            let config = directory_client::Config::new(self.config.directory_uri.clone());
            let directory: Arc<Client> = Arc::new(DirectoryClient::new(config));
            let listener_client = Arc::clone(&directory);
            tokio::spawn(async move {
                let mut listener = MixnetListener::new(
                    mixnet_receiver,
                    client_encryption_keypair,
                    listener_client,
                );
                listener.run().await;
            });

            self.sanity_check().await;
            self.test_all_nodes(directory).await;
            self.wait_for_interrupt().await
        });
    }

    async fn test_all_nodes(&mut self, directory: Arc<Client>) {
        let big_topology = directory
            .get_topology()
            .await
            .expect("couldn't retrieve topology from the directory server");

        let all_mixnodes = big_topology.mix_nodes.clone();
        let mut interval = time::interval(time::Duration::from_secs(2));
        let lastnode = all_mixnodes
            .last()
            .expect("No nodes in mixnode list. Exiting.")
            .to_owned();
        for mixnode in all_mixnodes {
            interval.tick().await;
            self.test_a_node(mixnode.to_owned()).await;
            if mixnode == lastnode {
                println!("we hit the last one");
            }
        }
    }

    async fn test_a_node(&mut self, mixnode: MixNodePresence) {
        println!("Testing mixnode: {}", mixnode.pub_key);
        let me = self.config.self_address.clone();
        let topology_to_test = good_topology::new_with_node(mixnode.clone());
        let message = mixnode.pub_key + ":4";
        let messages = self.prepare_messages(message, me, &topology_to_test);
        self.send_messages(messages).await;
    }

    /// Run some initial checks to ensure our subsequent measurements are valid.
    /// For example, we should be able to send ourselves a Sphinx packet (and receive it
    /// via the websocket, which currently fails.
    async fn sanity_check(&mut self) {
        let me = self.config.self_address.clone();
        let topology = &self.config.good_topology;

        let messages = self.prepare_messages("hello".to_string(), me, topology);
        self.send_messages(messages).await;
    }

    async fn send_messages(&mut self, socket_messages: Vec<(NymNodeRoutingAddress, SphinxPacket)>) {
        println!("foo");
        self.config
            .gateway_client
            .batch_send_sphinx_packets(socket_messages)
            .await
            .unwrap();
    }

    async fn wait_for_interrupt(&self) {
        if let Err(e) = tokio::signal::ctrl_c().await {
            error!(
                "There was an error while capturing SIGINT - {:?}. We will terminate regardless",
                e
            );
        }
        println!("Received SIGINT - the network monitor will terminate now");
    }
}
