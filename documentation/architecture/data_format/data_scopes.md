# Message payload management

Ockam Routing protocol defines payload as opaque data sent with messages.

Payload is used primarily to carry application data exchange.

Ockam Messaging describes workers behaviour mostly using routes,
but also introduces a concept of Message Metadata.

Metadata is a portion of a message which is not a part of application data,
but used to facilitate delivery.

For example, in indexed pipes each message is assigned an index.
While message data is preserved, message is extended with index metadata.

## Message data scopes

Let's say we have message exchange between application `A` and `B`
over some channel `C1, C2`

Application `A` would send messages to `OR: A->C1->B`, while `B` will receive messages
from `RR: A->C2->B`

**TODO: pictures**

Application code is aware of how messages are handled on `A->C1` and `C2->B`.

Message payload is treated by `C1,C2` as an opaque binary data.

Actual data layout is known only to `A` and `B`.
They can use some encoding protocol to encode application data
as binary and pass it to Ockam Messaging.

The application code is aware of the message protocol,
the fact that there is `A` or `B` and that there is a channel `C1,C2`.

Channel workers `C1` and `C2` are sending messages to each other,
they may use some additional message routes to get messages from `C1` to `C2`
and may use some additional metadata or message format between `C1` and `C2`

This means that channel (or pipe) workers to forward messages need to extend
the payload ot re-code the payload (e.g. secure channel encryption)

This way, messages on `A->C1` and `C2->B` would have one data format,
while messages on `C1->C2` would have another format.

We can say that messages on `A->C1` and `C2->B` exist on a different **scope** from messages on `C1->C2`.

Any additional messages sent between `C1` and `C2` (e.g. handshakes or confirmations) exist only in the channel scope. Those messages data format and routes are internal to the channel.

### Moving messages between scopes

Messages can be **passed to** a different scope, 
worker `C1` is passing it from application scope to the channel scope

Workers which pass messages between scopes, are called **message transforms**

Messages can be **forwarded through** a different scope and go back to the original scope,
original scope workers don't have to be aware of message structure and metadata used by the forwarding scope.

Message transforms which forward message through a scope are called **scope endpoints**

Endpoints can define the api protocol to forward messages, but usually they just accept binaries.

When forwarding through a scope, there is no requirement for messages leaving the scope to be exactly the same as they were entering the scope, but they must be in the same message format.

### Messaging endpoints 

When we send a message, in its delivery process there are multiple messages sent between potentially many workers.

Delivery workers can be classified as:

- "source" worker - the worker which creates the original message
- "destination" worker - the worker which message is supposed to be delivered to
- "intermediate"/"forwarders" - workers which pass the message from source to destination

Data used in original message created by the source may not come from the source itself, but some external system, but the message is created by the source.

Same way destination can pass the message data somewhere else.

Source forker is creating the message structure, intermediate workers may modify it and destination worker deconstructs the message and uses the data from it somehow.

We can apply this model to message scopes:

- "source" is a worker which creates a message in the scope format
- "destination" is a worker which deconstructs the scope message and uses the data
- "intermediate" workers may modify the message, but preserve the scope format

When forwarding a message through a forwarding scope, the scope endpoints would be sources and destinations of scope messages.

For example a pipe sender is a source of pipe scope messages and pipe receiver is a destination.

**TODO:** define entering/leaving scopes and scope wrapping when scopes may be internal to other scopes or external.

### Scope isolation

Since we might use multiple scopes in a pipelined delivery it's important to keep any scope-specific data in this scope only.

For example if one channel endpoint assigns indexes to messages, another endpoint should remove them. Otherwise workers further in delivery might wrongly interpret that data.

Or if messages were encrypted when entering a scope, they should be decrypted when leaving that scope.

## Data and metadata

Message delivery uses multiple formats which contain different information.

Some of this information is coming from the application code and some is added by the delivery workers.

In general we call information added by delivery "metadata" and information coming from outside - "data"

Since Ockam Messaging combines multiple layers in delivery, what we consider "data"
and "metadata" depends on the current scope.

Part of metadata only makes sense for a certain scope, while other may be useful across multiple scopes.

### Scope specific metadata

Scope specific metadata only valid within the same scope.

For example in a scope of an indexed pipe, the message index is scope specific.

When message is forwarded through a scope, the lower level scope metadata must be preserved as is to be recovered until the message goes back that that scope.

When message goes down the scope stack (exits the forwarding scope), the scope specific metadata for the forwarding scope should be stripped.

We don't define behaviour for metadata in non-stacked scopes.

### Scope agnostic metadata

Scope agnostic metadata is a portion of metadata which gets preserved when moving messages between scopes up and down the scope stack.

Scope agnostic metadata can be used for debugging or tracing of messages to trace the full path which messages are going through.









