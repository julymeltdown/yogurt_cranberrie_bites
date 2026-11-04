fn main() -> Result<(), Box<dyn std::error::Error>> {
    let relationship = contracts_build::proto_root().join("relationship/v1/relationship.proto");
    let protos = vec![
        relationship,
        contracts_build::proto_root().join("authority/v1/lease.proto"),
        contracts_build::proto_root().join("authority/v1/deletion_source.proto"),
        contracts_build::proto_root().join("concert/v1/account_deletion.proto"),
        contracts_build::source_auth_proto(),
        contracts_build::source_profile_proto(),
        contracts_build::source_friend_proto(),
        contracts_build::source_post_proto(),
        contracts_build::source_notification_proto(),
    ];
    let includes = vec![
        contracts_build::proto_root(),
        contracts_build::source_auth_include_dir(),
        contracts_build::source_profile_include_dir(),
        contracts_build::source_friend_include_dir(),
        contracts_build::source_post_include_dir(),
        contracts_build::source_notification_include_dir(),
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
