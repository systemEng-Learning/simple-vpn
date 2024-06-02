## tunnel

server command e.g

```
cargo run -- --server --ip-peer 172.26.0.3 --port 5005
```

client command setup e.g

```
cargo run -- --client --ip-peer 172.26.0.2 --local-port 5005
```

This used docker service to stimulate client and server

to get the service ip address e.g

```
docker container inspect  network-tuntap-client-1
```
