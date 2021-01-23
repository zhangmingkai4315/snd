# snd
snd is a dns traffic generator written by rust, it supports DNS over TCP/UDP and DOH. 
you can set almost every bit of the packet using arguments. 


#### 1. Usage

```
a dns traffic generator

USAGE:
snd [OPTIONS] [FLAGS]

OPTIONS:
    -s, --server <server>                          the dns server for benchmark [default: 8.8.8.8]
    -p, --port <port>                              the dns server port number [default: 53]
    -d, --domain <domain>                          domain name for dns query [default: example.com]
    -t, --type <qty>                               dns query type [default: A]
    -q, --qps <qps>                                dns query per second [default: 10]
    -m, --max <max>                                max dns packets will be send [default: 100]
    -c, --client <client>                          concurrent dns query clients [default: 10]
    -f, --file <file>                              the dns query file [default: ""]
        --edns-size <edns-size>                    set opt max EDNS buffer size [default: 1232]
        --protocol <protocol>                      the packet protocol for send dns request [default: UDP]
                                                   support protocols [UDP, TCP, DOT, DOH]
        --doh-server <doh-server>                  doh server based RFC8484 [default: https://dns.alidns.com/dns-query]
        --doh-server-method <doh-server-method>    doh http method[GET/POST] [default: GET]
        --source-ip <source>                       set the source ip address [default: 0.0.0.0]
        --timeout <timeout>                        timeout for wait the packet arrive [default: 5]
        --packet-id <packet-id>                    set to zero will random select a packet id [default: 0]
FLAGS:
        --check-all-message    default only check response header
        --debug                enable debug mode
        --disable-edns         disable EDNS
        --disable-rd           RD (recursion desired) bit in the query
        --enable-cd            CD (checking disabled) bit in the query
        --enable-dnssec        enable dnssec
        --file-loop            read dns query file in loop mode
HELP:
    -h, --help                 Prints help information
VERSION:
    -V, --version              Prints version information
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


##### DNS over DOT

- total query packets to 20
- query per second to 5
- dns server to dns.google(8.8.8.8)
- domain name to baidu.com
- domain type to NS

```
snd -m 20 -q 5 -s dns.google -p 853 -d baidu.com -t NS --protocol DOT

```

#### 2. Save Report

Using -o or --output save the result to file, if the filename end with ".json", it will print and save as json file; if the filename end with ".yaml" save as yaml file.

```
snd -m 20 -q 5 -s 8.8.8.8 -d google.com -t A --protocol tcp -o result.json
snd -m 20 -q 5 -s 8.8.8.8 -d google.com -t A -o result.yaml

```

#### 3. Query From File

Read dns query file instead set the query domain using -d argument.
Using -d will only allow one domain for each test, but query file mode allow send dns query line by line in loop mode.

Example of dns query.txt file: 
```
www.google.com A
www.facebook.com A
doubleclick.net NS
google-analytics.com SOA
akamaihd.net NS
googlesyndication.com NS
googleapis.com NS
googleadservices.com NS
www.youtube.com A
www.twitter.com A
scorecardresearch.com NS
microsoft.com NS
ytimg.com NS
```

Send query using query file mode:

```
snd -s 127.0.0.1 -m 200 -q 10 -c 1 -f query.txt

```


#### 3. Load Test 

Test Environments is a mac mini, install a local dns server for test. If you set up the environment at split dns server and snd load generator,
maybe can get much higher qps number.

```
Mac mini (2018)
3 GHz 6 Core Intel Core i5
32 GB 2667 MHz DDR4
DNS Server run at 127.0.0.1 / unbound 
```


```
------------   Report   --------------
     Total Cost : PT7.533013S (+time wait)
     Start Time : 2021-01-08T21:30:42.674911+08:00
       End Time : 2021-01-08T21:30:50.207924+08:00
    Total Query : 1000000
       Question : NS = 1000000
 Total Response : 1000000
  Response Code : No Error=1000000
   Success Rate : 100.00%
    Average QPS : 132749
    Min Latency : 18.025µs
    Max Latency : 100.793625ms
   Mean Latency : 55.011µs
    99% Latency : 86.39µs
    90% Latency : 64.415µs
    50% Latency : 52.762µs
```