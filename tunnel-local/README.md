### How To Use
Ensure that you have ip. This app setups your tun device with it.
#### Server
To run as a server, you can run it with the following optional config options:
* `--name` or `-n`: Name of the tun device you want to create. Default playtun
* `--port` or `-p`: UDP port. Default 2000
* `--key` or `-k`: Password for encryption and decryption
* `--local` or `l`: The IP address of the tun device you want to create.
To run a tunnel server with name simpletun, port 3456, device ip address 10.0.0.1 and password wordpass with cargo, it'd be like this
```sh
    cargo run -- --name simpletun --port 3456 --local 10.0.0.1  --key wordpass
```
Note that this will only create a tun device ip addr /24 subnet.

#### Client
Ensure you have iptables installed on your PC for you to run this package as a client. You can use the server options except `port`. Adding that option will setup a server. In addition, you must run with the following options.:
* `--client` or `-c`: Just calling this runs the tunnel as a client.
* `--address` or `-a`: Sets the ip address and port of the server.
* `--site-port` or `-s`: The port of the localhost server you want to tunnel packets to.
To run a client with name clienttun, tunnelserver 12.93.9.75:3456, device ip address 10.0.0.2 and password wordpass for a django site that runs on port 8000 with cargo, it'd be like this
```sh
    cargo run -- --client --name clienttun --address 12.93.9.75:3456 --local 10.0.0.2 --site-port 8000 --key wordpass
```

### Setup

#### Server Setup
You'd need to setup a web server that can receive request from users. I used nginx during development. In order to send request from nginx to my tun device. I setup a reverse proxy. The proxy conf for the above setup can be like this
```text
    server {
            listen 80;
            listen [::]:80;

            root /var/www/tunnel.example/html;
            index index.html index.htm index.nginx-debian.html;

            server_name tunnel.example www.tunnel.example;

            location / {
            proxy_pass http://10.0.0.2:8000;
            }
    }
```
Note that the `proxy_pass` is set to the client's tun device IP address. 

#### Client Setup
The client just needs you to setup a localhost server. For example, you can run a Django site like this
```sh
    python manage.py runserver
```

What happens after setting up will be like this:
User request -> Nginx -> Server tun device -> Client tun device -> NAT table via ip-translate -> Localhost server -> NAT table via ip-translate -> Client tun device -> Server tun device -> Nginx -> User.
I know that's long but I swear it works.


### Encryption
Your server and client must be running with the same password for successful encryption and decryption of packets.