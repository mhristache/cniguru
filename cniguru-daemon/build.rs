extern crate tower_grpc_build;

fn main() {
    // build the internal protocol
    tower_grpc_build::Config::new()
        .enable_server(true)
        .enable_client(true)
        .build(&["proto/cniguru/cniguru.proto"], &["proto/cniguru"])
        .unwrap_or_else(|e| panic!("protobuf compilation failed: {}", e));
}
