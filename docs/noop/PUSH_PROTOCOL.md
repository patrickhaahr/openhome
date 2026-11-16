# Self-hosted push protocol

This document specifies the wire contract for NOOP's **Experimental**, default-off export to a
user-owned HTTP(S) endpoint. Protocol version **1.0** covers the Android-first client. It is a
one-way export protocol: the on-device database is authoritative, the receiver acknowledges writes
and may advertise only which fixed v1 streams it accepts. NOOP never reads health data, commands,
URLs, field names, or other configuration back from the receiver.

NOOP does not ship, operate, or endorse a receiver. A receiver is not part of this repository, and
this contract must not be interpreted as an account, hosted-sync, restore, or two-way-sync API.

The contract is split across three pages.

- [Push protocol: transport and identity](PUSH_PROTOCOL_TRANSPORT.md) covers transport, authentication, identity and storage scope.
- [Push protocol: request and delivery](PUSH_PROTOCOL_DELIVERY.md) covers the NDJSON request, append delivery and rolling-window delivery.
- [Push protocol: streams, errors and versioning](PUSH_PROTOCOL_STREAMS_AND_ERRORS.md) covers the v1 stream registry, acceptance and errors, versioning and Apple compatibility.
