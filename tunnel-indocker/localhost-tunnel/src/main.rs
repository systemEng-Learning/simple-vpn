use clap::{ Parser, Subcommand};
use mio::event::Event;
use mio::net::{TcpListener, TcpStream};
use mio::{Events, Interest, Poll, Registry, Token};

use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::net::SocketAddr;

#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Cli {
    #[clap(subcommand)]
    command: Command,
}


#[derive(Subcommand, Debug)]
enum Command {
    Local {
        #[arg(short, long)]
        local_port: u16,

        #[arg(short, long)]
        server_ip: String,

        #[arg(short, long)]
        remote_port: u16
    },

    Server {
        #[clap(short, long)]
        local_port: u16
    },
}

pub fn main() {
    let cli = Cli::parse().command;

    match cli {
        Command::Local { local_port, server_ip, remote_port } => {
            let server_addr: SocketAddr = format!("{}:{}", server_ip, remote_port).parse().unwrap();
            let local_addr: SocketAddr = format!("127.0.0.1:{}", local_port).parse().unwrap();

            handle_client(server_addr, local_addr).unwrap();

        },
        Command::Server { local_port } => {
            handle_server(local_port).unwrap();
        }
    }

}

fn handle_client(server_addr: SocketAddr, local_addr: SocketAddr) -> std::io::Result<()> {
    const SERVER: Token = Token(0);
    const LOCAL: Token = Token(1);

    println!("Connecting to server: {:?}", server_addr);

    let mut server_stream = TcpStream::connect(server_addr)?;
    let mut local_stream = TcpStream::connect(local_addr)?;

    let mut poll = Poll::new()?;
    let mut events = Events::with_capacity(128);

    println!("Connected to server: {:?}", server_stream.peer_addr()?);

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

    loop {
        poll.poll(&mut events, None)?;

        for event in events.iter() {
            match event.token() {
                SERVER => {
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
                    let mut buffer = vec![0; 4096];
                    if let Err(e) = handle_connection(
                        poll.registry(),
                        &mut local_stream,
                        &mut server_stream,
                        event,
                        &mut buffer,
                    ) {
                        if e.kind() == io::ErrorKind::ConnectionReset {
                            println!("re-establishing connection....");
                            poll.registry().deregister(&mut local_stream)?;
                            poll.registry().deregister(&mut server_stream)?;
                            local_stream = TcpStream::connect(local_addr)?;
                            server_stream = TcpStream::connect(server_addr)?;

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
                        } else {
                            eprintln!("Error handling local stream: {:?}", e);
                            break;
                        }
                    }
                }
                _ => unreachable!(),
            }
        }
    }
}

fn handle_server(local_port: u16) -> std::io::Result<()> {
    const SERVER: Token = Token(0);
    const CLIENT: Token = Token(1);
    const LOCAL: Token = Token(2);
    const LOCALCON: Token = Token(3);

    let mut poll = Poll::new()?;
    let mut events = Events::with_capacity(128);

    let server_addr: SocketAddr = "0.0.0.0:8080".parse().unwrap();
    let mut server = TcpListener::bind(server_addr)?;
    println!("Listening on: {:?}", server.local_addr()?);

    poll.registry()
        .register(&mut server, SERVER, Interest::READABLE)?;

    println!("server registered");

    let mut connections: HashMap<Token, (TcpStream, TcpStream)> = HashMap::new();
    let mut connections2: HashMap<Token, (TcpListener, TcpStream)> = HashMap::new();

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

                        let local_port = local_port;
                        let local_addr: SocketAddr =
                            format!("0.0.0.0:{}", local_port).parse().unwrap();
                        let mut local_listener = TcpListener::bind(local_addr)?;

                        poll.registry().register(
                            &mut local_listener,
                            LOCALCON,
                            Interest::READABLE,
                        )?;

                        // connections.insert(CLIENT, (local_stream, client_stream));
                        connections2.insert(LOCALCON, (local_listener, client_stream));
                    }
                }

                LOCALCON => {
                    println!("GET TO READABLE HERE");

                    if let Some((local_listener, mut client_stream)) =
                        connections2.remove(&LOCALCON)
                    {
                        println!("LOCALHOST is in readable stage");
                        let (mut local_stream, address) = match local_listener.accept() {
                            Ok((local_stream, address)) => (local_stream, address),
                            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                                connections2.insert(LOCALCON, (local_listener, client_stream));
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
                            if e.kind() != io::ErrorKind::ConnectionReset {
                                eprintln!("Error handling local stream: {:?}", e);
                            }
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
                            eprintln!("Error handling client stream: {:?}", e);
                        }
                    }
                }
                _ => {
                    println!("Something reached here");
                }
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
                Ok(0) => {
                    return Err(io::Error::new(
                        io::ErrorKind::ConnectionReset,
                        "Connection closed",
                    ))
                }
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
                Ok(0) => {
                    return Err(io::Error::new(
                        io::ErrorKind::ConnectionReset,
                        "Connection closed",
                    ))
                }
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
