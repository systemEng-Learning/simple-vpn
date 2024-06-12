use clap::Parser;
use mio::event::Event;
use mio::net::{TcpListener, TcpStream};
use mio::{Events, Interest, Poll, Registry, Token};

use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::net::SocketAddr;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[arg(short, long)]
    client: bool,
    #[arg(short, long)]
    server: bool,
}

pub fn main() {
    let cli = Cli::parse();

    if cli.server && cli.client {
        eprintln!("Please specify either --client or --server, not both");
        return;
    }

    if cli.server {
        handle_server().unwrap();
    } else if cli.client {
        handle_client().unwrap();
    }
}

fn handle_client() -> std::io::Result<()> {
    // specify token to track server and local connections events
    const SERVER: Token = Token(0);
    const LOCAL: Token = Token(1);

    // specify server and local addresses
    let server_addr: SocketAddr = "172.26.0.3:8080".parse().unwrap(); // you can change this to your server address
    let local_addr: SocketAddr = "127.0.0.1:3001".parse().unwrap(); // also this to your local address

    println!("Connecting to server: {:?}", server_addr);

    // connect to server and local addresses
    let mut server_stream = TcpStream::connect(server_addr)?;
    let mut local_stream = TcpStream::connect(local_addr)?;

    // create a new poll instance
    let mut poll = Poll::new()?;
    let mut events = Events::with_capacity(128);

    println!("Connected to server: {:?}", server_stream.peer_addr()?);

    // register server and local streams to poll registry
    poll.registry().register(
        &mut server_stream,
        SERVER,
        Interest::READABLE | Interest::WRITABLE,
    )?;

    poll.registry().register(
        &mut local_stream,
        LOCAL,
        Interest::READABLE | Interest::WRITABLE,
    )?;

    let mut server_buffer = vec![0; 4096];
    let mut local_buffer = vec![0; 4096];

    loop {
        poll.poll(&mut events, None)?; // poll events

        // loop through events
        for event in events.iter() {
            match event.token() {
                SERVER => {
                    // handles any readabale and writable events on server stream
                    let mut buffer = vec![0; 4096];
                    if let Err(e) = handle_connection(
                        poll.registry(),
                        &mut server_stream,
                        &mut local_stream,
                        event,
                        &mut buffer,
                    ) {
                        eprintln!("Error handling server stream: {:?}", e);
                    }
                }
                LOCAL => {
                    // handles any readable and writable events on local stream
                    let mut buffer = vec![0; 4096];
                    if let Err(e) = handle_connection(
                        poll.registry(),
                        &mut local_stream,
                        &mut server_stream,
                        event,
                        &mut buffer,
                    ) {
                        eprintln!("Error handling local stream: {:?}", e);
                    }
                }
                _ => unreachable!(),
            }
        }
    }
}

fn handle_server() -> std::io::Result<()> {
    const SERVER: Token = Token(0); // token to track server events
    const CLIENT: Token = Token(1); // token to track client events
    const LOCAL: Token = Token(2); // token to track localhost connected client events
    const LOCALCON: Token = Token(3); // token to track localhost connection events

    let mut poll = Poll::new()?;
    let mut events = Events::with_capacity(128);

    let server_addr: SocketAddr = "0.0.0.0:8080".parse().unwrap();
    let mut server = TcpListener::bind(server_addr)?;
    println!("Listening on: {:?}", server.local_addr()?);

    poll.registry()
        .register(&mut server, SERVER, Interest::READABLE)?;

    println!("server registered");

    // manage connections
    let mut connections: HashMap<Token, (TcpStream, TcpStream)> = HashMap::new(); // store lochost:3200 stream mapping to client_locahost:3001 stream
    let mut connections2: HashMap<Token, (TcpListener, TcpStream)> = HashMap::new(); // store lohost listener to client_localhost:3001 stream

    loop {
        if let Err(err) = poll.poll(&mut events, None) {
            if interrupted(&err) {
                continue;
            }
            return Err(err);
        }

        for event in events.iter() {
            match event.token() {
                SERVER => {
                    // accept client connection
                    // and then create a local listener to accept localhost connection
                    if event.is_readable() {
                        println!("Accepting connection");
                        let (mut client_stream, address) = match server.accept() {
                            Ok((client_stream, address)) => (client_stream, address),
                            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                                break;
                            }
                            Err(e) => {
                                return Err(e);
                            }
                        };
                        println!("Accepted connection from: {:?}", address);

                        let local_port = 3200; // local port to listen to
                        let local_addr: SocketAddr =
                            format!("0.0.0.0:{}", local_port).parse().unwrap();
                        let mut local_listener = TcpListener::bind(local_addr)?;

                        // register local listener to poll registry
                        // so we can know when the browser is connected to localhost
                        poll.registry().register(
                            &mut local_listener,
                            LOCALCON,
                            Interest::READABLE,
                        )?;

                        connections2.insert(LOCALCON, (local_listener, client_stream));
                    }
                }

                LOCALCON => {
                    println!("GET TO READABLE HERE");
                    if event.is_readable() {
                        if let Some((local_listener, mut client_stream)) =
                            connections2.remove(&LOCALCON)
                        {
                            println!("LOCALHOST is in readable stage");
                            let (mut local_stream, address) = match local_listener.accept() {
                                Ok((local_stream, address)) => (local_stream, address),
                                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                                    break;
                                }
                                Err(e) => {
                                    return Err(e);
                                }
                            };

                            println!("Localhost Accepted connection from: {:?}", address);

                            poll.registry().register(
                                &mut client_stream,
                                CLIENT,
                                Interest::READABLE | Interest::WRITABLE,
                            )?;
                            poll.registry().register(
                                &mut local_stream,
                                LOCAL,
                                Interest::READABLE | Interest::WRITABLE,
                            )?;

                            connections.insert(CLIENT, (local_stream, client_stream));
                        }
                    }
                }
                LOCAL => {
                    println!("Local host now accessed properly");
                    if let Some((local_listener, client_stream)) = connections.get_mut(&CLIENT) {
                        let mut buffer = vec![0; 4096];
                        if let Err(e) = handle_connection(
                            poll.registry(),
                            local_listener,
                            client_stream,
                            event,
                            &mut buffer,
                        ) {
                            eprintln!("Error handling local stream: {:?}", e);
                        }
                    }
                }
                CLIENT => {
                    println!("Client sending streaks");
                    if let Some((local_listener, client_stream)) = connections.get_mut(&CLIENT) {
                        let mut buffer = vec![0; 4096];
                        if let Err(e) = handle_connection(
                            poll.registry(),
                            client_stream,
                            local_listener,
                            event,
                            &mut buffer,
                        ) {
                            eprintln!("Error handling local stream: {:?}", e);
                        }
                    }
                }
                _ => unreachable!(),
            }
        }
    }
}

fn handle_connection(
    registry: &Registry,
    src: &mut TcpStream,
    dst: &mut TcpStream,
    event: &Event,
    buffer: &mut [u8],
) -> io::Result<()> {
    if event.is_readable() {
        loop {
            match src.read(buffer) {
                Ok(n) if n > 0 => {
                    if let Err(e) = dst.write_all(&buffer[..n]) {
                        if would_block(&e) {
                            break;
                        } else if interrupted(&e) {
                            return handle_connection(registry, src, dst, event, buffer);
                        } else {
                            return Err(e);
                        }
                    }
                }
                Ok(0) => break,
                Ok(_) => break,
                Err(ref e) if would_block(e) => break,
                Err(ref e) if interrupted(e) => {
                    return handle_connection(registry, src, dst, event, buffer);
                }
                Err(e) => return Err(e),
            }
        }
    }

    if event.is_writable() {
        loop {
            match dst.read(buffer) {
                Ok(n) if n > 0 => {
                    if let Err(e) = src.write_all(&buffer[..n]) {
                        if would_block(&e) {
                            break;
                        } else if interrupted(&e) {
                            return handle_connection(registry, src, dst, event, buffer);
                        } else {
                            return Err(e);
                        }
                    }
                }
                Ok(0) => break,
                Ok(_) => break,
                Err(ref e) if would_block(e) => break,
                Err(ref e) if interrupted(e) => {
                    return handle_connection(registry, src, dst, event, buffer);
                }
                Err(e) => return Err(e),
            }
        }
    }
    Ok(())
}

fn next(current: &mut Token) -> Token {
    let next = current.0;
    current.0 += 1;
    Token(next)
}

fn interrupted(err: &io::Error) -> bool {
    err.kind() == io::ErrorKind::Interrupted
}

fn would_block(err: &io::Error) -> bool {
    err.kind() == io::ErrorKind::WouldBlock
}
