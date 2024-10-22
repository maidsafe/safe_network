## Identify Interval Decisions

###Status

Accepted

##Context

Idle nodes in a network of moderate data have a high ongoing bandwidth.

This appears to be because of the identify polling of nodes, which occurs at the default libp2p rate, of once per 5 minutes.

We see ~1mb/s traffic on nodes in a moderate network.

##Decision

We decrease the identify interval to once every hour. (It would be sent out on any node joining, and realistically things will not change too often here.)

##Consequences

This should reduce bandwidth requirements.

There could be unintended consequences if we rely on identify for anything after onboarding a node, but we currently do not.
