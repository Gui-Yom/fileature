use igd::aio::search_gateway;
use igd::{PortMappingProtocol, SearchOptions};
use std::env;
use std::net::{IpAddr, Ipv4Addr, SocketAddrV4, TcpListener};
use std::path::PathBuf;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

/// transfert
#[derive(Debug, argh::FromArgs)]
struct Args {
    /// server port number
    #[argh(option, short = 'p')]
    port: Option<u16>,
    /// do not forward port on router automatically
    #[argh(switch)]
    no_port_forward: bool,
    /// files to serve
    #[argh(positional)]
    files: Vec<PathBuf>,
}

#[tokio::main]
async fn main() {
    let args: Args = argh::from_env();

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            env::var("RUST_LOG").unwrap_or_else(|_| {
                "transfert_cli=debug,transfert_http=debug,tower_http=debug".into()
            }),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Unknown, we want the system to pick one for us
    let mut internal_port = args.port.unwrap_or(0);

    let socket = TcpListener::bind((Ipv4Addr::new(0, 0, 0, 0), internal_port)).unwrap();

    // System generated port
    internal_port = socket.local_addr().unwrap().port();

    if args.no_port_forward {
        println!("files: {:#?}", args.files);
        tracing::info!("Server address is http://127.0.0.1:{internal_port}");
        fileature_http::run_server(socket, &args.files).await;
    } else {
        // Local interface ip addr
        let ip = match local_ip_address::local_ip() {
            Ok(IpAddr::V4(v4)) => v4,
            err => unreachable!("got {err:?}"),
        };

        let gw = search_gateway(SearchOptions {
            bind_addr: SocketAddrV4::new(ip, 0).into(),
            ..Default::default()
        })
        .await
        .expect("Can't find gateway on local network");

        let external_port = match gw
            .add_any_port(
                PortMappingProtocol::TCP,
                SocketAddrV4::new(ip, internal_port),
                3600 * 24,
                "transfert",
            )
            .await
        {
            Ok(external) => {
                tracing::info!(
                    "Automatically forwarded port {internal_port} to {external} on router"
                );
                external
            }
            Err(err) => {
                tracing::error!("Error : {err}");
                tracing::info!("Failed to automatically forward port on router.");
                tracing::info!("You will probably have to do it manually.");
                internal_port
            }
        };
        let external_ip = gw.get_external_ip().await.expect("Can't get external ip");
        tracing::info!("Server address is http://{external_ip}:{external_port}");

        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                tracing::debug!("Received CTRL+C");
                gw.remove_port(PortMappingProtocol::TCP, external_port).await.unwrap()
            }
            _ = fileature_http::run_server(socket, &args.files) => {}
        }
    }
}
