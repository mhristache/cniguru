extern crate bytes;
extern crate env_logger;
extern crate http;
extern crate futures;
extern crate log;
extern crate prost;
#[macro_use]
extern crate prost_derive;
extern crate tokio_core;
extern crate tower_h2;
extern crate tower_http;
extern crate tower_grpc;

use futures::Future;
use tokio_core::reactor::Core;
use tokio_core::net::TcpStream;
use tower_grpc::Request;
use tower_h2::client::Connection;

pub mod cniguru_proto {
    include!(concat!(env!("OUT_DIR"), "/cniguru_proto.rs"));
}

pub fn main() {
    let _ = ::env_logger::init();

    let mut core = Core::new().unwrap();
    let reactor = core.handle();

    let addr = "[::1]:50051".parse().unwrap();
    let uri: http::Uri = format!("http://localhost:50051").parse().unwrap();

    let veth_pairs = TcpStream::connect(&addr, &reactor)
        .and_then(move |socket| {
            // Bind the HTTP/2.0 connection
            Connection::handshake(socket, reactor)
                .map_err(|_| panic!("failed HTTP/2.0 handshake"))
        })
        .map(move |conn| {
            use cniguru_proto::client::IntProto;
            use tower_http::add_origin;

            let conn = add_origin::Builder::new()
                .uri(uri)
                .build(conn)
                .unwrap();

            IntProto::new(conn)
        })
        .and_then(|mut client| {
            use cniguru_proto::{GetVethIntfPairsReq, ContainerRuntime};

            client.get_veth_intf_pairs(Request::new(GetVethIntfPairsReq {
                container_id: "some id".to_string(),
                runtime: ContainerRuntime::Docker as i32,
            })).map_err(|e| panic!("gRPC request failed; err={:?}", e))
        })
        .and_then(|response| {
            println!("RESPONSE = {:?}", response);
            Ok(())
        })
        .map_err(|e| {
            println!("ERR = {:?}", e);
        });

    core.run(veth_pairs).unwrap();
}
