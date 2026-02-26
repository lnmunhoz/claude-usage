use axum::Router;
use axum_server::tls_rustls::RustlsConfig;
use std::{env, net::SocketAddr, path::PathBuf};
use tower_http::services::ServeDir;

#[tokio::main]
async fn main() {
    // Serve from tools/update-server/dist (next to this binary's Cargo.toml)
    let project_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let serve_dir = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| project_dir.join("dist"));

    let serve_dir = serve_dir
        .canonicalize()
        .expect("serve directory does not exist — run a build first to populate dist/");

    let cert_dir = env::var("CERT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| project_dir.join("certs"));

    let cert = cert_dir.join("localhost+1.pem");
    let key = cert_dir.join("localhost+1-key.pem");

    if !cert.exists() || !key.exists() {
        eprintln!("TLS certs not found in {}", cert_dir.display());
        eprintln!(
            "Generate them with:\n  mkcert -cert-file {dir}/localhost+1.pem -key-file {dir}/localhost+1-key.pem localhost 127.0.0.1",
            dir = cert_dir.display()
        );
        std::process::exit(1);
    }

    let tls_config = RustlsConfig::from_pem_file(&cert, &key)
        .await
        .expect("failed to load TLS config");

    let app = Router::new().fallback_service(ServeDir::new(&serve_dir));

    let addr = SocketAddr::from(([0, 0, 0, 0], 8443));
    println!("Serving {} on https://localhost:8443", serve_dir.display());

    axum_server::bind_rustls(addr, tls_config)
        .serve(app.into_make_service())
        .await
        .expect("server error");
}
