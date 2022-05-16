use anyhow::{Context, Result};
use hyper::{
    client::{HttpConnector, ResponseFuture},
    header::{HeaderValue, HOST},
    server::conn::AddrStream,
    service::{make_service_fn, service_fn},
    Body, Client, Request, Response, Server,
};
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

    let server = Server::bind(&server_addr.as_str().parse::<SocketAddr>().unwrap()).serve(make_svc);

    if let Err(e) = server.await {
        error!("server error: {}", e);
    }

    Ok(())
}

pub async fn service(mut request: Request<Body>) -> Result<Response<Body>> {
    // modify uri

    let mut uri = request.uri_mut();

    *uri = CONFIG.get_uri(&uri).context("no URI matched")?;

    let host = uri.host().unwrap().to_string();

    let mut headers = request.headers_mut();

    headers.get_mut(HOST).map(|h| *h = host.parse::<HeaderValue>().unwrap());

    info!("sending request: {:?}", request);

    Ok(CLIENT.with(|c| c.request(request)).await?)
}

#[allow(dead_code)]
pub fn create_bad_request_response() -> String {
    format!(
        "\
    HTTP/1.1 400 Bad Request\
    server: hps\
    connection: closed\
    \n\n\
<!DOCTYPE>
<html>
<head>
    <title>Bad request</title>
</head>
<body><pre>
    Bad request!
</pre></body>
</html>"
    )
}
