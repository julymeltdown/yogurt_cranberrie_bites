fn main() -> Result<(), Box<dyn std::error::Error>> {
    let protos = [
        contracts_build::source_notification_proto(),
        contracts_build::source_authority_proto(),
    ];
    let include_dir = contracts_build::source_notification_include_dir();
    let protoc = protoc_bin_vendored::protoc_bin_path()?;
    unsafe {
        std::env::set_var("PROTOC", protoc);
    }

    tonic_prost_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(&protos, &[include_dir])?;

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
