use anyhow::Result;
use clap::{error::ErrorKind, CommandFactory, Parser, Subcommand};
use locbore::{client::Client, server::Server};

#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Args {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Starts a local proxy to the remote server.
    Local {
        /// The local port to expose.
        local_port: u16,

        /// The local host to expose.
        #[clap(short, long)]
        local_host: String,

        /// Address of the remote server to expose local ports to.
        #[clap(short, long)]
        to: String,

        /// Optional port on the remote server to select.
        #[clap(short, long)]
        port: u16,
    },

    /// Runs the remote proxy server.
    Server {
        /// Minimum accepted TCP port number.
        #[clap(long, default_value_t = 1024)]
        port: u16,
    },
}

#[tokio::main]
async fn run(command: Command) -> Result<()> {
    match command {
        Command::Local {
            local_host,
            local_port,
            to,
            port,
        } => {
            let client = Client::new(&local_host, local_port, &to, port).await?;
            client.listen().await?;
        }
        Command::Server { port } => {
            Server::new(port).listen().await?;
        }
    }

    Ok(())
}

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    run(Args::parse().command)
}
