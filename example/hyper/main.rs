use std::net::ToSocketAddrs;
use hyper::body::Bytes;
use hyper::header::{RANGE, HOST, CONTENT_RANGE, LOCATION};

use http_body_util::BodyExt;
use tokio::io::{AsyncWriteExt, AsyncSeekExt, SeekFrom};
use tokio_rustls::rustls;
use tokio_stream::StreamExt;

const ALPN_H2: &str = "h2";
const CHUNK_SIZE: usize = 100_000_000;

const URL: &str = "https://huggingface.co/ByteDance/Hyper-SD/resolve/main/Hyper-SDXL-1step-Unet-Comfyui.fp16.safetensors";
const FILE: &str = "fp16.safetensors";

async fn download_chunk(u: hyper::Uri, s: usize, e: usize) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let host = u.host().expect("no host");
    let port = u.port_u16().unwrap_or(443);
    let addr = format!("{}:{}", host, port).to_socket_addrs()?.next().unwrap();

    let conf = std::sync::Arc::new({
        let root_store = rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        let mut c = rustls::ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth();
        c.alpn_protocols.push(ALPN_H2.as_bytes().to_owned());
        c
    });

    let tcp = tokio::net::TcpStream::connect(&addr).await?;
    let domain = rustls_pki_types::ServerName::try_from(host)?.to_owned();;
    let connector = tokio_rustls::TlsConnector::from(conf);

    let stream = connector.connect(domain, tcp).await?;
    let _io = hyper_util::rt::TokioIo::new(stream);
    let exec = hyper_util::rt::tokio::TokioExecutor::new();

    let (mut client, mut h2) = hyper::client::conn::http2::handshake(exec, _io).await?;
    tokio::spawn(async move {
        if let Err(e) = h2.await {
            println!("Error: {:?}", e);
        }
    });

    let range = format!("bytes={s}-{e}");

    let req = hyper::Request::builder()
        .uri(u)
        .header("user-agent", "hyper-client-http2")
        .header(RANGE, range)
        .version(hyper::Version::HTTP_2)
        .body(http_body_util::Empty::<Bytes>::new())?;

    let mut response = client.send_request(req).await?;
    
    let mut file = tokio::fs::OpenOptions::new().write(true).open(FILE).await?;
    file.seek(SeekFrom::Start(s as u64)).await?;    
    while let Some(chunk) = response.frame().await {
        let chunk = chunk?;
        if let Some(c) = chunk.data_ref() {
            tokio::io::copy(&mut c.as_ref(), &mut file).await?;
        }
    }
    Ok(())
}

async fn download() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut url: hyper::Uri = URL.parse()?;
    let host = url.host().expect("no host");
    let port = url.port_u16().unwrap_or(443);
    let addr = format!("{}:{}", host, port).to_socket_addrs()?.next().unwrap();

    let conf = std::sync::Arc::new({
        let root_store = rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        let mut c = rustls::ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth();
        c.alpn_protocols.push(ALPN_H2.as_bytes().to_owned());
        c
    });

    let tcp = tokio::net::TcpStream::connect(&addr).await?;
    let domain = rustls_pki_types::ServerName::try_from(host)?.to_owned();;
    let connector = tokio_rustls::TlsConnector::from(conf);

    let stream = connector.connect(domain, tcp).await?;
    let _io = hyper_util::rt::TokioIo::new(stream);
    let exec = hyper_util::rt::tokio::TokioExecutor::new();

    let (mut client, mut h2) = hyper::client::conn::http2::handshake(exec, _io).await?;
    tokio::spawn(async move {
        if let Err(e) = h2.await {
            println!("Error: {:?}", e);
        }
    });

    let req = hyper::Request::builder()
        .uri(url.clone())
        .header("user-agent", "hyper-client-http2")
        .header(RANGE, "bytes=0-0")
        .version(hyper::Version::HTTP_2)
        .body(http_body_util::Empty::<Bytes>::new())?;

    let mut response = client.send_request(req).await?;
    while let Some(location) = response.headers().get(LOCATION) {
        let _cdn: hyper::Uri = location.to_str()?.parse()?;
        let _req = hyper::Request::builder()
            .uri(_cdn.clone())
            .header("user-agent", "hyper-client-http2")
            .version(hyper::Version::HTTP_2)
            .body(http_body_util::Empty::<Bytes>::new())?;
        response = client.send_request(_req).await?;
        url = _cdn;
    }

    println!("{:?}", url);
    let req = hyper::Request::builder()
        .uri(url.clone())
        .header("user-agent", "hyper-client-http2")
        .header(RANGE, "bytes=0-0")
        .version(hyper::Version::HTTP_2)
        .body(http_body_util::Empty::<Bytes>::new())?;
    let response = client.send_request(req).await?;

    println!("{:?}", response);
    let length: usize = response
        .headers()
        .get(CONTENT_RANGE)
        .ok_or("Content-Length not found")?
        .to_str()?.rsplit('/').next()
        .and_then(|s| s.parse().ok())
        .ok_or("Failed to parse size")?;

    let _ = tokio::fs::File::create(FILE).await?.set_len(length as u64).await?;
    let tasks: Vec<_> = (0..length)
        .into_iter()
        .step_by(CHUNK_SIZE)
        .map(|s| {
            let _url = url.clone();
            let e = std::cmp::min(s + CHUNK_SIZE - 1, length);
            tokio::spawn(async move { download_chunk(_url, s, e).await })
        })
        .collect();

    for task in tasks {
        let _ = task.await.unwrap();
    }
    Ok(())
}

fn main() {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let start_time = std::time::Instant::now();
    let _ = rt.block_on(download());
    println!("Processing time: {:?}", start_time.elapsed());
}

