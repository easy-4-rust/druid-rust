fn main() -> Result<(), Box<dyn std::error::Error>> {
    let protoc = protoc_bin_vendored::protoc_bin_path()?;
    std::env::set_var("PROTOC", &protoc);

    tonic_build::configure()
        .build_server(false)
        .build_client(false)
        .compile_protos(&["proto/druid_metrics_v1.proto"], &["proto"])?;

    Ok(())
}
