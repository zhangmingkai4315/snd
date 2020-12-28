# snd
snd is a dns traffic generator written by rust, it supports DNS over TCP/UDP and DOH. 
you can set almost every bit of the packet using arguments. 


### 1. Usage


```
snd 0.1.0
a dns traffic generator

USAGE:
    snd [FLAGS] [OPTIONS]

FLAGS:
        --check-all-message    default only check response header
        --debug                enable debug mode
        --disable-edns         disable edns
        --disable-rd           RD (recursion desired) bit in the query
        --enable-cd            CD (checking disabled) bit in the query
        --enable-dnssec        enable dnssec
    -h, --help                 Prints help information
    -V, --version              Prints version information

OPTIONS:
    -c, --client <client>                          concurrent dns query clients [default: 10]
        --doh-server <doh-server>                  doh server based RFC8484 [default: https://dns.alidns.com/dns-query]
        --doh-server-method <doh-server-method>    doh http method[GET/POST] [default: GET]
    -d, --domain <domain>                          domain name for dns query [default: example.com]
        --edns-size <edns-size>                    edns size for dns packet and receive buffer [default: 1232]
    -m, --max <max>                                max dns packets will be send [default: 100]
        --packet-id <packet-id>                    set to zero will random select a packet id [default: 0]
    -p, --port <port>                              the dns server port number [default: 53]
        --protocol <protocol>                      the packet protocol for send dns request [default: UDP]
    -q, --qps <qps>                                dns query per second [default: 10]
    -t, --type <qty>                               dns query type [multi types supported] [default: A,SOA]
    -s, --server <server>                          the dns server for benchmark [default: 8.8.8.8]
        --source-ip <source>                       set the source ip address [default: 0.0.0.0]
        --timeout <timeout>                        timeout for wait the packet arrive [default: 5]


```

##### DNS over UDP 

- total query packets to 20
- query per second to 5
- dns server to 8.8.8.8
- domain name to google.com
- domain type to NS 
  
```
snd -m 20 -q 5 -s 8.8.8.8 -d google.com -t NS

```


##### DNS over TCP

- total query packets to 20
- query per second to 5
- dns server to 8.8.8.8
- domain name to google.com
- domain type to A 
- using tcp protocol 


```
snd -m 20 -q 5 -s 8.8.8.8 -d google.com -t A --protocol tcp
```

##### DoH

- total query packets to 20
- query per second to 5
- domain name to google.com
- domain type to A 
- using doh protocol
- set doh server to https://cloudflare-dns.com/dns-query

More information please read the rfc8484, currently snd only support basic doh 
and no json style support.

```
snd -m 20 -q 5 -d google.com -t NS --protocol DOH --enable-dnssec --debug --doh-server=https://cloudflare-dns.com/dns-query
```