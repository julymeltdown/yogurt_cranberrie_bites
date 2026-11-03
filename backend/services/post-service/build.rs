fn main() -> Result<(), Box<dyn std::error::Error>> {
    let post_include = contracts_build::source_post_include_dir();
    let protos = vec![
        post_include.join("post/v1/post.proto"),
        contracts_build::source_authority_proto(),
        post_include.join("friend/v1/friend.proto"),
        post_include.join("notification/v1/notification.proto"),
        post_include.join("profile/v1/profile.proto"),
    ];
    let protoc = protoc_bin_vendored::protoc_bin_path()?;
    unsafe {
        std::env::set_var("PROTOC", protoc);
    }

    tonic_prost_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(&protos, &[post_include])?;

    for proto in protos {
        println!("cargo:rerun-if-changed={}", proto.display());
    }
    Ok(())
}
