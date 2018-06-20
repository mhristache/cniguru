#[test]
fn test_parse_ip_link_printout_basic() {
    use super::{parse_ip_link_printout, Intf};

    let s = r#"1: lo: <LOOPBACK,UP,LOWER_UP> mtu 65536 qdisc noqueue state UNKNOWN mode DEFAULT group default qlen 1000
    link/loopback 00:00:00:00:00:00 brd 00:00:00:00:00:00
2: vethc3cef48b@if3: <BROADCAST,MULTICAST,UP,LOWER_UP> mtu 1450 qdisc noqueue master cni0 state UP mode DEFAULT group default
    link/ether e6:93:28:78:39:99 brd ff:ff:ff:ff:ff:ff link-netnsid 0
3: enp0s31f6: <NO-CARRIER,BROADCAST,MULTICAST,UP> mtu 1500 qdisc fq_codel state DOWN mode DEFAULT group default qlen 1000
    link/ether c8:5b:76:72:53:46 brd ff:ff:ff:ff:ff:ff
4: wlp3s0: <BROADCAST,MULTICAST,UP,LOWER_UP> mtu 1500 qdisc mq state UP mode DORMANT group default qlen 1000
    link/ether e4:a7:a0:61:3d:3e brd ff:ff:ff:ff:ff:ff
9: docker0: <NO-CARRIER,BROADCAST,MULTICAST,UP> mtu 1500 qdisc noqueue state DOWN mode DEFAULT group default
    link/ether 02:42:1b:7f:0d:5e brd ff:ff:ff:ff:ff:ff
11: wwp0s20f0u5c2: <BROADCAST,MULTICAST> mtu 1500 qdisc noop state DOWN mode DEFAULT group default qlen 1000
    link/ether 02:1e:10:1f:00:00 brd ff:ff:ff:ff:ff:ff
12: flannel.1: <BROADCAST,MULTICAST,UP,LOWER_UP> mtu 1450 qdisc noqueue state UNKNOWN mode DEFAULT group default
    link/ether da:1f:7a:e1:59:58 brd ff:ff:ff:ff:ff:ff
13: cni0: <BROADCAST,MULTICAST,UP,LOWER_UP> mtu 1450 qdisc noqueue state UP mode DEFAULT group default qlen 1000
    link/ether 5a:02:70:6b:57:1e brd ff:ff:ff:ff:ff:ff
14: veth551a254e@if3: <BROADCAST,MULTICAST,UP,LOWER_UP> mtu 1450 qdisc noqueue master cni0 state UP mode DEFAULT group default
    link/ether 12:56:7d:9f:80:15 brd ff:ff:ff:ff:ff:ff link-netnsid 1"#;

    let exp = vec![
        Intf {
            name: "veth551a254e".into(),
            bridge: Some("cni0".into()),
            mtu: 1450,
            mac_address: "12:56:7d:9f:80:15".into(),
        },
    ];

    let got = parse_ip_link_printout(s, 1).unwrap();

    assert_eq!(exp, got);
}

#[test]
fn test_parse_ip_link_printout_multus() {
    use super::{parse_ip_link_printout, Intf};

    let s = r#"610: veth987c7292@if5: <BROADCAST,MULTICAST,UP,LOWER_UP> mtu 1500 qdisc noqueue master bla-bla-int0 state UP mode DEFAULT group default
    link/ether 46:ed:60:c6:e9:73 brd ff:ff:ff:ff:ff:ff link-netnsid 6"#;

    let exp = vec![
        Intf {
            name: "veth987c7292".into(),
            bridge: Some("bla-bla-int0".into()),
            mtu: 1500,
            mac_address: "46:ed:60:c6:e9:73".into(),
        },
    ];

    let got = parse_ip_link_printout(s, 6).unwrap();

    assert_eq!(exp, got);
}
