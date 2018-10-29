extern crate bytes;
extern crate env_logger;
extern crate futures;
#[macro_use]
extern crate log;
extern crate prost;
#[macro_use]
extern crate prost_derive;
extern crate tokio_core;
extern crate tower_h2;
extern crate tower_grpc;


pub mod cniguru_proto {
    include!(concat!(env!("OUT_DIR"), "/cniguru_proto.rs"));
}

use cniguru_proto::{server, GetVethIntfPairsReq, GetVethIntfPairsRes, VethIntf, VethIntfPair};

use futures::{future, Future, Stream};
use tokio_core::net::TcpListener;
use tokio_core::reactor::Core;
use tower_h2::Server;
use tower_grpc::{Request, Response};


#[derive(Clone, Debug)]
struct CniInfoGetter;

impl server::IntProto for CniInfoGetter {
    type GetVethIntfPairsFuture = future::FutureResult<Response<GetVethIntfPairsRes>, tower_grpc::Error>;

    fn get_veth_intf_pairs(&mut self, request: Request<GetVethIntfPairsReq>) -> Self::GetVethIntfPairsFuture {
        println!("REQUEST = {:?}", request);

        // get a reference to the message included in the request
        let _msg = request.get_ref();

        let response = Response::new(GetVethIntfPairsRes {
            container_pid: 0,
            interfaces: vec![
                VethIntfPair {
                    container: Some(VethIntf {
                        name: "c".to_string(),
                        ifindex: 0,
                        peer_ifindex: 0,
                        mtu: 1500,
                        mac_address: "foo".to_string(),
                        bridge: String::default(),
                        ip_address: String::default(),
                    }),
                    node: Some(VethIntf {
                        name: "n".to_string(),
                        ifindex: 0,
                        peer_ifindex: 0,
                        mtu: 1500,
                        mac_address: "foo".to_string(),
                        bridge: String::default(),
                        ip_address: String::default(),
                    }),
                }
            ],
        });

        future::ok(response)
    }
}

pub fn main() {
    let _ = ::env_logger::init();

    let mut core = Core::new().unwrap();
    let reactor = core.handle();

    let new_service = server::IntProtoServer::new(CniInfoGetter);

    let h2 = Server::new(new_service, Default::default(), reactor.clone());

    let addr = "[::1]:50051".parse().unwrap();
    let bind = TcpListener::bind(&addr, &reactor).expect("bind");

    let serve = bind.incoming()
        .fold((h2, reactor), |(h2, reactor), (sock, _)| {
            if let Err(e) = sock.set_nodelay(true) {
                return Err(e);
            }

            let serve = h2.serve(sock);
            reactor.spawn(serve.map_err(|e| error!("h2 error: {:?}", e)));

            Ok((h2, reactor))
        });

    core.run(serve).unwrap();
}
