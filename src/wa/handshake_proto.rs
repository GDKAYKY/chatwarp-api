/// Minimal handshake envelope used for M2 tests.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct HandshakeMessage {
    #[prost(bytes = "vec", tag = "1")]
    pub client_ephemeral: ::prost::alloc::vec::Vec<u8>,
    #[prost(bytes = "vec", tag = "2")]
    pub server_ephemeral: ::prost::alloc::vec::Vec<u8>,
    #[prost(bytes = "vec", tag = "3")]
    pub encrypted_static: ::prost::alloc::vec::Vec<u8>,
    #[prost(bytes = "vec", tag = "4")]
    pub payload: ::prost::alloc::vec::Vec<u8>,
}
