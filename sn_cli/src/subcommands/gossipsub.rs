// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use clap::Subcommand;
use color_eyre::Result;
use sn_client::{Client, ClientEvent};

#[derive(Subcommand, custom_debug::Debug)]
pub enum GossipsubCmds {
    /// Subscribe to a topic and listen for messages published on it
    Subscribe {
        /// The name of the topic.
        #[clap(name = "topic")]
        topic: String,
    },
    /// Unsubscribe from a topic
    Unsubscribe {
        /// The name of the topic.
        #[clap(name = "topic")]
        topic: String,
    },
    /// Publish a message on a given topic
    Publish {
        /// The name of the topic.
        #[clap(name = "topic")]
        topic: String,
        /// The message to publish.
        #[clap(name = "msg")]
        #[debug(skip)]
        msg: String,
    },
}

pub(crate) async fn gossipsub_cmds(cmds: GossipsubCmds, client: &Client) -> Result<()> {
    match cmds {
        GossipsubCmds::Subscribe { topic } => {
            client.subscribe_to_topic(topic.clone())?;
            println!("Subscribed to topic '{topic}'. Listening for messages published on it...");
            let mut events_channel = client.events_channel();
            while let Ok(event) = events_channel.recv().await {
                if let ClientEvent::GossipsubMsg { msg, .. } = event {
                    let msg = String::from_utf8(msg.to_vec())?;
                    println!("New message published: {msg}");
                }
            }
        }
        GossipsubCmds::Unsubscribe { topic } => {
            client.unsubscribe_from_topic(topic.clone())?;
            println!("Unsubscribed from topic '{topic}'.");
        }
        GossipsubCmds::Publish { topic, msg } => {
            client.publish_on_topic(topic.clone(), msg.into())?;
            println!("Message published on topic '{topic}'.");
        }
    }
    Ok(())
}
