fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto = contracts_build::source_profile_proto();
    let include_dir = contracts_build::source_profile_include_dir();
    let protoc = protoc_bin_vendored::protoc_bin_path()?;
    unsafe {
        std::env::set_var("PROTOC", protoc);
    }

    tonic_prost_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(
            &[proto, contracts_build::source_authority_proto()],
            &[include_dir],
        )?;

    println!(
        "cargo:rerun-if-changed={}",
        contracts_build::source_profile_proto().display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        contracts_build::source_authority_proto().display()
    );
    Ok(())
}
