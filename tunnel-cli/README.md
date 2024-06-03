### How To Use
#### Server
To run as a server, you can run it with the following optional config options:
* `--name` or `-n`: Name of the tun device you want to create. Default playtun
* `--port` or `-p`: UDP port. Default 2000
* `--key` or `-k`: Password for encryption and decryption
To run a tunnel server with name simpletun, port 3456, and password wordpass with cargo, it'd be like this
```sh
    cargo run -- --name simpletun --port 3456 --key wordpass
```
That's it

#### Client
To run as a client you must run with the following options:
* `--client` or `-c`: Just calling this runs the tunnel as a client.
* `--address` or `-a`: Sets the ip address and port of the server.
To run a client with name clienttun, tunnelserver 12.93.9.75:3456, and password, it'd be like this
```sh
    cargo run -- --client --name clienttun --address 12.93.9.75:3456
```

### Setup
After running the above, you'd need to set up the different parameters on the tunnel devices. Let's say we want the server to have tun0 ip address to be 10.0.0.1 and client's own to be 10.0.0.2 and we want to be able to access example.com (93.184.215.14) via the tunnel. This is how it will be done

#### Server Setup
```sh
    ip link set dev simpletun up # Activate tun device
    ip addr add 10.0.0.1/24 dev simpletun   # Add address for our tun device
    echo 1 > /proc/sys/net/ipv4/ip_forward  # Activate IP forwarding
    iptables -t nat -A POSTROUTING -s 10.0.0.0/24 -j MASQUERADE
```
The nat command replaces the source address of packets with source range 10.0.0.0-10.0.0.255 to that of the server public ip address. Then packets sent in response to the outgoing packets have their destination set back to the previous one. It is important to activate IP forwarding in order for your device to act as a router

#### Client Setup
```sh
    ip link set dev clienttun up
    ip addr add 10.0.0.2/24 dev clienttun
    route add -host 93.184.215.14/32 gw 10.0.0.2 dev clienttun
```
The route command routes example.com ip through our tunnel.