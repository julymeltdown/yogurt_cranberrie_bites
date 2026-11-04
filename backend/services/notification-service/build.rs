fn main() -> Result<(), Box<dyn std::error::Error>> {
    let protoc = protoc_bin_vendored::protoc_bin_path()?;
    unsafe {
        std::env::set_var("PROTOC", protoc);
    }

    let lease = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../proto/authority/v1/lease.proto");
    let local_include = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../proto");
    let authority_output =
        std::path::PathBuf::from(std::env::var("OUT_DIR")?).join("owner_authority");
    std::fs::create_dir_all(&authority_output)?;
    tonic_prost_build::configure()
        .build_server(true)
        .build_client(true)
        .out_dir(&authority_output)
        .compile_protos(&[lease.clone()], &[local_include.clone()])?;
    let notification = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../proto/notification/v1/concert_inbox.proto");
    let protos = [
        notification.clone(),
        contracts_build::source_notification_proto(),
        contracts_build::source_authority_proto(),
    ];
    let include_dir = contracts_build::source_notification_include_dir();

    tonic_prost_build::configure()
        .build_server(true)
        .build_client(true)
        .extern_path(".authority.lease.v1", "crate::concert::transport::lease")
        .compile_protos(&protos, &[local_include, include_dir])?;

    println!("cargo:rerun-if-changed={}", lease.display());
    println!("cargo:rerun-if-changed={}", notification.display());
    println!(
        "cargo:rerun-if-changed={}",
        contracts_build::source_notification_proto().display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        contracts_build::source_authority_proto().display()
    );
    Ok(())
}
