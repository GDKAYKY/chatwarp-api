fn main() {
    let protoc = protoc_bin_vendored::protoc_bin_path().expect("failed to find vendored protoc");
    unsafe {
        std::env::set_var("PROTOC", protoc);
    }

    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(&["proto/whatsapp_v2.proto"], &["proto"])
        .expect("failed to compile protos");
}
