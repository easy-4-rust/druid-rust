fn main() -> Result<(), Box<dyn std::error::Error>> {
    let protoc = protoc_bin_vendored::protoc_bin_path()?;
    std::env::set_var("PROTOC", &protoc);

    let build_client = std::env::var_os("CARGO_FEATURE_CLIENT").is_some();
    let build_server = std::env::var_os("CARGO_FEATURE_SERVER").is_some();
    tonic_prost_build::configure()
        // RPC 名为 `Connect`，因此关闭 Tonic 同名 transport 构造器；
        // 客户端改为使用 `MetricsIngestClient::new(channel)` 构造。
        .build_transport(false)
        .build_client(build_client)
        .build_server(build_server)
        .compile_protos(&["proto/druid_metrics_v1.proto"], &["proto"])?;

    Ok(())
}
