# stableset_net

![Alt text](assets/ant.png?raw=true "Here come the ants")

This is an advanced implementation of the stableset experiment. In this iteration we use no signatures and depend upon an IP based certificate authority (rustls will release an update for us to provide this). This no signature approach has certain advantages and disadvantages. The main advantage is that it is much faster and easier to implement. With the burden of signatures removed then nodes should also be much leaner and less cpu bound. 

## Outline

### Nodes and their identities 

1. Nodes are identified by their name, IP address and port number.
2. Node names are 32 byte values and these will become teh public keys of n IP based Public Key Infrastructure (PKI).
3. This PKI, secures connections between nodes and proves their authenticity to each other.
4. The above removes any man in the middle attacks and allows for a much simpler and faster implementation. 
5. Nodes are connected to each other via QUIC.
6. Nodes communicate directly with each other (no proxy).

### Consensus 

1. Signatures are replaced by *witnesses*. 
2. We cannot proxy authority around the network as we can with signatures, which is not necessarily a deficiency.
3. We directly communicate with the eldest nodes in our stableset and ask them to witness our observations.
4. When we see majority of elders have witnessed our observation, we can be sure that it is correct and they will also update their stableset or network state to match ours.
5. The stableset identifier (hash of members) is used as an anti entropy mechanism. This means nodes will communicate differences in their stableset to each other.

### Stableset configuration and use 

1. The stableset struct contains the identifier, set of members, set of leaving members and set of joining members. 
2. The stableset identifier is a hash of the members set.
3. When there is a discrepancy via Ae, the whole stableset is sent in both directions, i.e. to receiver and sender
4. Nodes can merge their stableset to come to the same state or to notice that one of them has not enough witness for a node leave or join, which is fine.

### Witnesses and no sigs 

1. We use a witness struct to store the witness information.
2. We require a majority **of who we believe are the eldest contactable nodes** (our elders) to witness our observation.

Signatures would allow us to proxy authority around the network. This is not possible with witnesses. This is a good thing as it means we can be sure that the node we are communicating with is the node we think it is and receive it's most current view of the state. With signatures this could be an older version of the state.

Point 2 above is very important as we almost immediately promote and adult (member node) to an elder. This immediate reconfiguration of our network view allows us to reduce the elder size to 4. This is a very small number of nodes to have to contact to get a majority of witnesses and will means we can reach decisions between elders in 24 messages. If those messages are small and have no cpu requirement on validating a crypto sig then we reach decisions quickly. Those decisions may be as drastic as we have lost all elders in one network outage.  

For this reason with section we gossip the top 20 members of the stableset to all other sections. This means that if we lose all elders in one network outage then we can immediately see the new elders and use those. 

