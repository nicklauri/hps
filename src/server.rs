use anyhow::{Context, Result};
use bytesize::ByteSize;
use hyper::{
    client::{HttpConnector, ResponseFuture},
    header::{HeaderValue, CONTENT_LENGTH, HOST},
    server::conn::AddrStream,
    service::{make_service_fn, service_fn},
    Body, Client, Request, Response, Server,
};
use once_cell::sync::Lazy;
use std::{convert::Infallible, net::SocketAddr, time::Instant};
use tokio::signal;
use tracing::{error, info};

use crate::{config::CONFIG, util};

static CLIENT: Lazy<Client<HttpConnector>> = Lazy::new(Client::default);

pub async fn run() -> Result<()> {
    let make_svc = make_service_fn(|socket: &AddrStream| {
        let remote_addr = socket.remote_addr();
        async move { Ok::<_, Infallible>(service_fn(move |req| service(req, remote_addr))) }
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

pub async fn service(mut request: Request<Body>, addr: SocketAddr) -> Result<Response<Body>> {
    let mut uri = request.uri_mut();

    *uri = CONFIG.get_uri(&uri).context("no URI matched")?;

    let host = uri.host().map(HeaderValue::from_str).context("no host")??;

    let mut headers = request.headers_mut();

    headers.get_mut(HOST).map(|h| *h = host);

    if CONFIG.verbose {
        info!("sending request: {:#?}", request);
    }

    let method = request.method().to_string();
    let uri = request
        .uri()
        .path_and_query()
        .map(|s| s.to_string())
        .unwrap_or_else(String::new);
    let timer = Instant::now();

    let response = CLIENT.request(request).await?;

    let elapsed = timer.elapsed();

    let status = response.status();
    let content_length = response
        .headers()
        .get(CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        .map(ByteSize);

    if let Some(content_length) = content_length {
        info!(
            "{} {:>7} {} -- {} - {} - after: {:?}",
            addr, method, uri, status, content_length, elapsed
        );
    } else {
        info!("{} {:>7} {} -- {} - after: {:?}", addr, method, uri, status, elapsed);
    }

    Ok(response)
}

pub async fn shutdown_signal() {
    signal::ctrl_c().await.expect("install Ctrl-C signal handler");

    info!("server is shutting down.");
}
