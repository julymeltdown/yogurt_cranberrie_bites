fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto = contracts_build::source_concert_proto();
    let protoc = protoc_bin_vendored::protoc_bin_path()?;
    unsafe {
        std::env::set_var("PROTOC", protoc);
    }
    tonic_prost_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(&[proto.clone()], &[contracts_build::proto_root()])?;
    println!("cargo:rerun-if-changed={}", proto.display());
    Ok(())
}
