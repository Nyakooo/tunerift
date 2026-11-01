use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};
use symphonia::core::{
    codecs::DecoderOptions, formats::FormatOptions, io::MediaSourceStream, meta::MetadataOptions,
    probe::Hint,
};
use tunerift::stream::RadioReader;

struct Server {
    address: String,
    seen: Arc<Mutex<Vec<String>>>,
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}
fn headers(socket: &mut TcpStream) -> String {
    let mut data = vec![];
    loop {
        let mut b = [0];
        if socket.read_exact(&mut b).is_err() {
            break;
        }
        data.push(b[0]);
        if data.ends_with(b"\r\n\r\n") || data.len() > 16000 {
            break;
        }
    }
    String::from_utf8_lossy(&data).into()
}
impl Server {
    fn new(socks: bool) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap().to_string();
        let seen = Arc::new(Mutex::new(vec![]));
        let requests = seen.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let done = stop.clone();
        let worker = thread::spawn(move || {
            while !done.load(Ordering::Relaxed) {
                let Ok((mut socket, _)) = listener.accept() else {
                    thread::sleep(Duration::from_millis(5));
                    continue;
                };
                socket
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                if socks {
                    let mut hello = [0; 2];
                    socket.read_exact(&mut hello).unwrap();
                    assert_eq!(hello[0], 5);
                    let mut methods = vec![0; hello[1] as usize];
                    socket.read_exact(&mut methods).unwrap();
                    socket.write_all(&[5, 0]).unwrap();
                    let mut connect = [0; 4];
                    socket.read_exact(&mut connect).unwrap();
                    assert_eq!(connect[0], 5);
                    match connect[3] {
                        3 => {
                            let mut len = [0];
                            socket.read_exact(&mut len).unwrap();
                            let mut host = vec![0; len[0] as usize];
                            socket.read_exact(&mut host).unwrap();
                            requests
                                .lock()
                                .unwrap()
                                .push(String::from_utf8(host).unwrap());
                        }
                        1 => {
                            let mut ip = [0; 4];
                            socket.read_exact(&mut ip).unwrap();
                        }
                        _ => panic!("unsupported SOCKS address"),
                    };
                    let mut port = [0; 2];
                    socket.read_exact(&mut port).unwrap();
                    socket.write_all(&[5, 0, 0, 1, 127, 0, 0, 1, 0, 0]).unwrap();
                }
                let request = headers(&mut socket);
                let target = request
                    .lines()
                    .next()
                    .unwrap_or("")
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or("")
                    .to_string();
                requests.lock().unwrap().push(target.clone());
                let path = if target.starts_with("http") {
                    url::Url::parse(&target).unwrap().path().to_string()
                } else {
                    target
                };
                let (kind,body):( &str,Vec<u8>)=match path.as_str(){"/master.m3u8"=>("application/vnd.apple.mpegurl",b"#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=64000\nmedia.m3u8\n".to_vec()),"/media.m3u8"=>("application/vnd.apple.mpegurl",b"#EXTM3U\n#EXT-X-VERSION:3\n#EXT-X-TARGETDURATION:2\n#EXT-X-MEDIA-SEQUENCE:0\n#EXTINF:2,\ntone.ts\n#EXT-X-ENDLIST\n".to_vec()),"/tone.ts"=>("video/mp2t",include_bytes!("fixtures/tone.ts").to_vec()),_ =>("audio/mpeg",include_bytes!("fixtures/tone.mp3").to_vec())};
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = socket.write_all(response.as_bytes());
                let _ = socket.write_all(&body);
            }
        });
        Self {
            address,
            seen,
            stop,
            thread: Some(worker),
        }
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            t.join().unwrap();
        }
    }
}
fn decode(url: &str, proxy: &str) -> usize {
    let reader = RadioReader::open(url, proxy, Arc::new(AtomicBool::new(false))).unwrap();
    let mss = MediaSourceStream::new(Box::new(reader), Default::default());
    let mut format = symphonia::default::get_probe()
        .format(
            &Hint::new(),
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .unwrap()
        .format;
    let t = format.default_track().unwrap();
    let id = t.id;
    let mut decoder = symphonia::default::get_codecs()
        .make(&t.codec_params, &DecoderOptions::default())
        .unwrap();
    let mut frames = 0;
    while let Ok(packet) = format.next_packet() {
        if packet.track_id() == id {
            let audio = decoder.decode(&packet).unwrap();
            frames += audio.frames();
        }
    }
    assert!(frames > 40000, "decoded {frames} frames");
    frames
}
#[test]
fn direct_mp3_decodes() {
    let server = Server::new(false);
    decode(&format!("http://{}/tone.mp3", server.address), "");
}
#[test]
fn hls_master_and_segments_use_http_proxy() {
    let server = Server::new(false);
    decode(
        "http://unresolvable.invalid/master.m3u8",
        &format!("http://{}", server.address),
    );
    let seen = server.seen.lock().unwrap();
    assert!(seen.iter().any(|s| s.ends_with("/master.m3u8")));
    assert!(seen.iter().any(|s| s.ends_with("/media.m3u8")));
    assert!(seen.iter().any(|s| s.ends_with("/tone.ts")));
}
#[test]
fn socks5h_remote_dns_and_audio() {
    let server = Server::new(true);
    decode(
        "http://radio-unresolvable.invalid/tone.mp3",
        &format!("socks5h://{}", server.address),
    );
    assert!(
        server
            .seen
            .lock()
            .unwrap()
            .iter()
            .any(|s| s == "radio-unresolvable.invalid")
    );
}
#[test]
fn stop_cancels_stalled_network() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let cancel = Arc::new(AtomicBool::new(false));
    let c = cancel.clone();
    let worker =
        thread::spawn(move || RadioReader::open(&format!("http://{address}/"), "", c).is_err());
    let (_socket, _) = listener.accept().unwrap();
    let start = std::time::Instant::now();
    cancel.store(true, Ordering::Relaxed);
    assert!(worker.join().unwrap());
    assert!(start.elapsed() < Duration::from_secs(1));
}

#[test]
fn https_uses_connect_proxy_and_fails_closed() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let worker = thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let request = headers(&mut socket);
        socket
            .write_all(
                b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            )
            .unwrap();
        request
    });
    assert!(
        RadioReader::open(
            "https://no-such-radio.invalid/live",
            &format!("http://{address}"),
            Arc::new(AtomicBool::new(false))
        )
        .is_err()
    );
    assert!(
        worker
            .join()
            .unwrap()
            .starts_with("CONNECT no-such-radio.invalid:443 ")
    );
}
#[test]
fn directory_client_supports_authenticated_http_proxy() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let worker = thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let request = headers(&mut socket);
        socket
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n[]")
            .unwrap();
        request
    });
    let client = tunerift::directory::client(&format!("http://user:password@{address}")).unwrap();
    assert_eq!(
        client
            .get("http://no-such-directory.invalid/stations")
            .send()
            .unwrap()
            .text()
            .unwrap(),
        "[]"
    );
    let request = worker.join().unwrap().to_lowercase();
    assert!(request.contains("proxy-authorization: basic dxnlcjpwyxnzd29yza=="));
    assert!(request.contains("http://no-such-directory.invalid/stations"));
}
