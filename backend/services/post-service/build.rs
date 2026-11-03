fn main() -> Result<(), Box<dyn std::error::Error>> {
    let post_include = contracts_build::source_post_include_dir();
    let protos = vec![
        post_include.join("post/v1/post.proto"),
        post_include.join("post/v1/concert_feed.proto"),
        contracts_build::source_authority_proto(),
        post_include.join("friend/v1/friend.proto"),
        post_include.join("notification/v1/notification.proto"),
        post_include.join("profile/v1/profile.proto"),
    ];
    let protoc = protoc_bin_vendored::protoc_bin_path()?;
    unsafe {
        std::env::set_var("PROTOC", protoc);
    }

    // Generate the canonical owner IDL once; imported Feed envelopes must use
    // this crate-local type instead of generating incompatible nested copies.
    let lease = post_include.join("authority/v1/lease.proto");
    tonic_prost_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(&[lease.clone()], &[post_include.clone()])?;
    tonic_prost_build::configure()
        .build_server(true)
        .build_client(true)
        .extern_path(
            ".authority.lease.v1",
            "crate::concert_feed::authority_proto",
        )
        .compile_protos(&protos, &[post_include])?;
    println!("cargo:rerun-if-changed={}", lease.display());

    for proto in protos {
        println!("cargo:rerun-if-changed={}", proto.display());
    }
    Ok(())
}
