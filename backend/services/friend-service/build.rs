fn main() -> Result<(), Box<dyn std::error::Error>> {
    let protos = vec![
        contracts_build::source_authority_proto(),
        contracts_build::source_friend_proto(),
        contracts_build::source_profile_proto(),
    ];
    let includes = vec![
        contracts_build::source_friend_include_dir(),
        contracts_build::source_profile_include_dir(),
    ];
    let protoc = protoc_bin_vendored::protoc_bin_path()?;
    unsafe {
        std::env::set_var("PROTOC", protoc);
    }
    tonic_prost_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(&protos, &includes)?;
    for proto in protos {
        println!("cargo:rerun-if-changed={}", proto.display());
    }
    Ok(())
}
