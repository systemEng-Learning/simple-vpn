## VPN tunnel in /vpn

server command e.g

```
cargo run -- --server --ip-peer 172.26.0.3 --port 5005
```

After running this you need to do the following

```
echo 1 > /proc/sys/net/ipv4/ip_forward  # Activate IP forwarding
iptables -t nat -A POSTROUTING -s 10.0.0.0/24 -j MASQUERADE
```

Also for a reason, since I was using docker the packet sent out was not really getting outside, so we had to apply masquerading to the eth0 of the docker

```
iptables -t nat -A POSTROUTING -o eth0 -j MASQUERADE
```

client command setup e.g

```
cargo run -- --client --ip-peer 172.26.0.2 --local-port 5005
```

also, you can test by routing this ip to our `tun-client`

```
route add -host 93.184.215.14/32 gw 10.0.0.2 dev tun-client
```

once that is done you will be able to ping the IP 
```
ping 93.184.215.14

or

curl -4  http://93.184.215.14/
```


This used docker service to stimulate client and server

to get the service ip address e.g

```
docker container inspect  network-tuntap-client-1
```
