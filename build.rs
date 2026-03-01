fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=proto/wa_md.proto");

    let protoc = protoc_bin_vendored::protoc_bin_path().expect("failed to locate protoc binary");
    // SAFETY: build script runs in a single process and this env mutation is local
    // to build-script execution before prost-build reads PROTOC.
    unsafe {
        std::env::set_var("PROTOC", protoc);
    }

    prost_build::Config::new()
        .compile_protos(&["proto/wa_md.proto"], &["proto"])
        .expect("failed to compile wa_md.proto");
}
