## Replication Interval Decisions

###Status

Accepted

##Context

Idle nodes in a network of moderate data have a high ongoing bandwidth.

This appears to be because of the replication of records going out every _interval_.

We see ~1mb/s traffic on nodes in a moderate network.

##Decision

We increased the max interval to 450s from 45s, to effectively reduce the bandwidth usage ten-fold.

##Consequences

This should reduce bandwidth requirements.

This may make replication tests slower, but we could adjust the interval for testing.
