Overview
--------

`cniguru` is a tool that can be used to troubleshoot containers networking.
It provides information about node interfaces used by docker and kubernetes containers:
- the name, MAC address and MTU of the host interfaces used by containers
- the bridge the interfaces are connected to

License
-------

Licensed under either of

* Apache License, Version 2.0, (LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license (LICENSE-MIT or http://opensource.org/licenses/MIT)

at your option.

Examples
--------

* List the interfaces for a kubernetes pod in a human readable format:

```bash
[root@kh1 ~]# KUBECONFIG=/etc/kubernetes/admin.conf kubectl get pod
NAME                              READY     STATUS    RESTARTS   AGE
netshoot-57c7994b66-zxdsl         1/1       Running   0          2m
serve-hostname-86bc9d96dc-n7n6h   1/1       Running   0          7d
[root@kh1 ~]# 
[root@kh1 ~]# cniguru pod netshoot-57c7994b66-zxdsl 

CONTAINER_ID  PID    NODE  INTF(C)  MAC_ADDRESS(C)     IP_ADDRESS(C)    INTF(N)       BRIDGE(N)
3e08cafbb6eb  26393  kh1   eth0     0a:58:0a:f4:00:de  10.244.0.222/24  veth0c97cb60  cni0
3e08cafbb6eb  26393  kh1   net0     0a:58:0a:08:08:06  10.8.8.6/24      veth74689fd2  br_dc_test

```

* Present the output in JSON format:

```bash
[root@kh1 ~]# cniguru pod netshoot-57c7994b66-zxdsl -o json
[
  {
    "container": {
      "id": "3e08cafbb6eb01558e86ba53f170b62855f0bf5a328a77dc2da278061ff7fdc8",
      "pid": 26393,
      "node_name": "kh1",
      "runtime": "Docker"
    },
    "interfaces": [
      {
        "container": {
          "name": "eth0",
          "ifindex": 3,
          "peer_ifindex": 558,
          "mtu": 1460,
          "mac_address": "0a:58:0a:f4:00:de",
          "bridge": null,
          "ip_address": "10.244.0.222/24"
        },
        "node": {
          "name": "veth0c97cb60",
          "ifindex": 558,
          "peer_ifindex": 3,
          "mtu": 1460,
          "mac_address": "0a:20:94:a0:35:64",
          "bridge": "cni0",
          "ip_address": null
        }
      },
      {
        "container": {
          "name": "net0",
          "ifindex": 5,
          "peer_ifindex": 559,
          "mtu": 1500,
          "mac_address": "0a:58:0a:08:08:06",
          "bridge": null,
          "ip_address": "10.8.8.6/24"
        },
        "node": {
          "name": "veth74689fd2",
          "ifindex": 559,
          "peer_ifindex": 5,
          "mtu": 1500,
          "mac_address": "d2:ae:0b:9f:62:72",
          "bridge": "br_dc_test",
          "ip_address": null
        }
      }
    ]
  }
]
```

Installation
------------

### Cargo

If you have a rust toolchain setup you can install `cniguru` via cargo:

```
cargo install cniguru
```

### Build from sources

* Make sure you have a newer Rust compiler installed. Run

```
rustup override set stable
rustup update stable
```


* Clone the source code:

```
git clone https://github.com/maximih/cniguru
cd cniguru
```

* Build `cniguru`

```
cargo build --release
```

### Download `x86_64` binary

A statically linked binary for linux `x86_64` is provided [here](https://github.com/maximih/cniguru/releases/download/0.2.0/cniguru_x86_64_0.2.0.tar.gz)

Configuration
-------------

The path to the Kubernetes config can be set via `$KUBECONFIG` env variable.
If `$KUBECONFIG` is not set, `cniguru` will try to use `$HOME/.kube/config` or `/etc/kubernetes/admin.conf`.

Docker related info is fetched using `docker` cli so `cniguru` must be run with an user that has rights to execute docker commands.

Future plans
------------

Some features that might be added in the future:

* run `cniguru` as a kubernetes `daemonset` (server side) and access it via a REST API (client side)
* add possibility to trace packets on different interfaces and different nodes
* add possibility to enable packet logging
