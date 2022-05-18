use anyhow::{Context, Result};
use hyper::{
    client::{HttpConnector, ResponseFuture},
    header::{HeaderValue, HOST},
    server::conn::AddrStream,
    service::{make_service_fn, service_fn},
    Body, Client, Request, Response, Server,
};
use tokio::signal;
use std::{convert::Infallible, net::SocketAddr};
use tracing::{error, info};

use crate::config::CONFIG;

thread_local! {
    static CLIENT: Client<HttpConnector> = Client::default();
}

pub async fn run() -> Result<()> {
    let make_svc = make_service_fn(|socket: &AddrStream| {
        let remote_addr = socket.remote_addr();
        async move { Ok::<_, Infallible>(service_fn(service)) }
    });

    let server_addr = format!("{}:{}", CONFIG.server_addr, CONFIG.server_port);

    let server = Server::bind(&server_addr.as_str().parse::<SocketAddr>().unwrap())
        .serve(make_svc)
        .with_graceful_shutdown(shutdown_signal());

    if let Err(e) = server.await {
        error!("server error: {}", e);
    }

    Ok(())
}

pub async fn service(mut request: Request<Body>) -> Result<Response<Body>> {
    info!("{:<5} {}", request.method().as_str(), request.uri());

    let mut uri = request.uri_mut();

    *uri = CONFIG.get_uri(&uri).context("no URI matched")?;

    let host = uri.host().map(HeaderValue::from_str).context("no host")??;

    let mut headers = request.headers_mut();

    headers.get_mut(HOST).map(|h| *h = host);

    if CONFIG.verbose {
        info!("sending request: {:#?}", request);
    }

    Ok(CLIENT.with(|c| c.request(request)).await?)
}

pub async fn shutdown_signal() {
    signal::ctrl_c().await.expect("install Ctrl-C signal handler");

    info!("server is shutting down.");
}
