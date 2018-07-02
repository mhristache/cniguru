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
netshoot-6d994df756-v9rgf         1/1       Running   0          2m
serve-hostname-86bc9d96dc-8cb49   1/1       Running   0          2m
[root@kh1 ~]# 
[root@kh1 ~]# ./cniguru pod netshoot-6d994df756-v9rgf

CONTAINER_ID  NODE  INTERFACE     MTU   MAC_ADDRESS        BRIDGE
7180b102b955  kh1   vethe8c8302a  1460  a6:eb:27:39:dd:dd  cni0
7180b102b955  kh1   veth593daf68  1500  e2:31:16:b3:66:40  br_dc_test
```

* Present the output in JSON format:

```bash
$ sudo cniguru pod serve-hostname-86bc9d96dc-9b8xn -o json
[{"container":{"id":"994ae42819bb8f7311f4e0d89cd83a5499ed02008b34091010e73329f1707a0b","pid":23256,"node_name":"sentinel","runtime":"Docker"},"interfaces":[{"container":{"name":"eth0","ifindex":3,"peer_ifindex":14,"mtu":1500,"mac_address":"3e:f0:1f:0f:27:ae","bridge":null,"ip_address":"10.244.0.5/24"},"node":{"name":"veth20ac475f","ifindex":14,"peer_ifindex":3,"mtu":1500,"mac_address":"7a:86:33:d4:33:bf","bridge":"cni0","ip_address":null}}]}]
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

A statically linked binary for linux `x86_64` is provided [here](https://github.com/maximih/cniguru/releases/download/0.1.0/cniguru_x86_64_0.1.0.tar.gz)

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
