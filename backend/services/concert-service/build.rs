fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto = contracts_build::source_concert_proto();
    let world_proto = contracts_build::source_world_proto();
    let post_concert_proto = contracts_build::proto_root().join("post/v1/concert_feed.proto");
    let protoc = protoc_bin_vendored::protoc_bin_path()?;
    unsafe {
        std::env::set_var("PROTOC", protoc);
    }
    tonic_prost_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(
            &[proto.clone(), world_proto.clone(), post_concert_proto.clone()],
            &[contracts_build::proto_root()],
        )?;
    println!("cargo:rerun-if-changed={}", proto.display());
    println!("cargo:rerun-if-changed={}", world_proto.display());
    println!("cargo:rerun-if-changed={}", post_concert_proto.display());
    Ok(())
}
