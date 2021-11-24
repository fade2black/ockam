# Message structure breakdown

## Message routing information

Routing message layer carries:

- routing information (onward route)
- backtracing information (return route)
- payload (must be opaque to the routing level)

Node router uses:

- Address message was routed to (using first address from onward route)
- Address message was sent from (access control)

Workers use:

- Address message was routed to

Forwarding workers use:

- Address message was routed to 
- onward route to forward message

Backtracing forwarding workers use:

- Address message was routed to
- return route to trace return route

Indirect information:

Message destination (final address in onward route?) - not necessary
Whether the current worker is destination

Message source (final address in return route?) - not necessary

Missing information:

Whether message return route is traced by previous workers
Whether message return route may be used (was properly traced)


Metadata:

Missing routing bits can be added to the message payload as metadata,
for example additional workers in the pipe channel handshake

In stream messages reply stream is in metadata, while it's actually is a routing
information



## Worker types 

- Endpoint workers

Act as message sources and destinations
Encoding message payloads as certain protocols between source and destination

- Forwarding workers

Use message routing information to forward message
Transform message routing information to forward message and trace return route
MAY use internal worker state to calculate new routes combined or instead of original message routes

Don't modify message payloads

- Message transforms

MAY act as forwarding workers

Modify message payload in some way

- Payload wrappers/unwrappers

Subset of message transforms
Wraps original message payload with additional metadata

Must forward message to a worker which will unwrap the message

- Message wrappers/unwrappers

Wrappers which don't only wrap payload, but routing information as well

Message wrappers act as endpoint workers to each other

Routing between wrapper and unwrapper USUALLY not takend from the message

## Route scope

An onward route [A, B, C] terminates on C

A return route [D, E, F] originates from F

Routing OR:[A, B, C]; RR:[D, E, F] originates from F and terminates on C

Forwarding workers in the middle may modify that, but well behaving workers supposed to deliver message to C



Routes MAY contain addresses from different scopes!!!

[A1, B1, C2, D2] - A1 and B1 are on node 1 and C2, D2 are on node 2

This means that worke B1 should move message between scopes.

BUT this message routing operates on 2 scopes.


The message itself may be delivered over addresses in multiple scopes, for example B1 may wrap the message, send ot over scope 3, then worker in scope 2 somehow gets the wrapped message and unwraps it in scope 2.

Delivery B1->C2 may also extend the route to [B1, E3, F4, C2], this message would be extended to scopes 3 and 4, but these routes may not be traced or worker C2 may remove scopes 3 and 4 from the tracing and message forwarded by C2 would also be only over scopes 1 and 2

It's not recommended to extend messages to more scopes, instead message may be wrapped and delivered over extra scopes and then unwrapped.

Alternatively, the added scopes must be removed when returning to the original message scopes.

If we don't use wrapping then it's easy to expand the routes but may not be easy to collapse them because that would mean removing parts of the return route (or onward route) without a necessary knowledge on which exact portion of the route should be removed.


Discovery and static remote addresses

Let's say we have a worker A1 on node 1 and we have a worker B2 and node2

There is a way to deliver messages from 1 to 2 over some means:
to deliver from 1 to 2 we use cloud node 3 (forwarder F3)
to deliver from 2 to 1 we use cloud node 4 (forwarder F4)

Let's say we know a unidirectional transport addresses (T13, T24) and static forwarder addresses

A1->B2 will be: OR: [T13, F3, B2], RR: [A1]
B2->A1 will be: OR: [T24, F4, A1], RR: [B2]

To make it easier to deal with transport and forwarder addresses, we can add local discovery workers on 1 and 2 which will make sure forwarding works and set up transport connections.
We call them D1 and D2, D for Discovery

These workers would isolate cloud communication.

A1->B2 will be: OR: [D1, B2], RR: [A1]
B2->A1 will be: OR: [D2, A1], RR: [B2]

These are the outgoing workers. We can make them incoming workers as well by having F3 and F4 point to them, then if they're getting forwarded messages they can also trace return routes:


A message sent from D1:
OR: [T13, F3, B2], RR: [A1]

Gets forwarded to D2 as:
OR: [D2, B2], RR: [A1]

Assuming transport and forwarding are not tracing routes, if transport does trace the routes, then:
OR: [D2, B2], RR: [(T23, F3), A1]

The discovery worker would remove the transport tracing when forwarding the message to B2:
OR: [B2], RR: [D2, A1]

As you can see, this is a backtrace of B2->A1


Discovery worker must be aware of how the routes are traced and where does the cloud tracing ends to remove it.

Alternatively discovery workers can wrap the "forwarded message" `OR: [B2], RR: [A1]` and instead forward:
OR: [T13, F3], RR: []

This will be received by D2 as:
OR: [D2], RR: [(T23, F3)]

Which can ignore the return route and unwrap the message: `OR: [B2], RR: [A1]` and trace itself in the return route:
OR: [B2], RR: [D2, A1]



## Message metadata approach

Metadata can be added to the message base structure

If metadata is generic, metadata keys need to be added and encoded as a map

If metadata is specific, we need to know wrapping/unwrapping endpoints and messages have to go through such endpoints to be unwrapped

Metadata encoding overhead:

Bare encoding provides efficient format to encode maps with a format like `{:map, :string, :data}`
We can't use non-string keys because it's not possible to make number codes generic.

String key overhead is 1 + length of the string bytes

Map overhead is 1 bytes.

If we have a map with N keys with summary string length of L, then overhead of using generic metadata in a message instead of wrapping a message is: `L + N + 1`


Another part of the metadata challenge: layers of communication (e.g. channels) might have conflicting keys for the metadata which should be preserved somehow. We might end up wrapping the metadata.
And this includes the routing information, as we want to preserve the original message routes and those will most definitely be conflicting.


**In secure channels do we want to encrypt payload or metadata as well?**
**Should metadata be a part of payload?**


If we wrap messages as payloads, an unwrap worker becomes a scope gateway. Message in one scope enters and message in another scope leaves. Within the wrapped context the message of the outer scope (wrapped message) is a payload - it does not carry useful information.



### A case for metadata

Metadata which is not a part of the payload makes sense when the metadata information should be shared with ALL workers in the path of the message (maybe within the single message scope)

This might be debug information, sender and receiver information, tracing information etc.

Metadata is not validated by workers and can be modified by workers of the same scope, so it can be lost, accessed and corrupted.

### Metadata as a part of payload VS metadata as a part of the message

If metadata is a part of the payload, it gets forwarded and encrypted same way as the payload, so there is no need to change existing workers to add it and there is no need to implement metadata forwarding as long as payload is forwarded.
This may also be addressed by forwarding workers always using the same forwarding APIs to construct new messages

The downside is that workers depends on whether they need to decode metadata or not will need to unwrap metadata from the payload.

Workers also will have to unwrap the data part of the payload.

**This makes it an explicit decision of the worker whether to unwrap metadata/data pair**
Drawing a line between forwarding workers, which treat entire payload as a single thing and the endpoint workers which need the data. **Although forwarding workers might need metadata**

Alternative will be to have metadata be a first level message property.

This means forwarding workers will have to forward metadata same way as they forward payload, **but maybe they can do tracing in there, so they will be changing it as they change the routes**

This makes extracting data form the payload easier though.

This also means that metadata forwarding public information if it's not wrapped in the payload, because secure channel will not encrypt it.

Forwariding workers will have to have some sort of rules regarding handling metadata in regards to what they can add or remove from there.

For example tracing information could be treated in such way.

Wrapping workers may chose to wrap the metadata of propagate it to the wrapped context, or maybe propagate only part of it into the wrapped context. Then unwrapping workers may chose to expand the metadata with the wrapped context metadata or use the one from the payload. I can see a use for both.

So for thie tracing and debug case metadata would better live in the message, while for pipes and channels internal metadata it would better be in the payload because it's the internal endpoint worker protocol, which is what payloads are for and it needs to be layerable and not leak outside of the pipe scope.





