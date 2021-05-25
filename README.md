# snd
snd is a dns traffic generator written by rust, it supports DNS over TCP/UDP and DOH. 
you can set almost every bit of the packet using arguments. 


#### 1. Usage

```
# ./target/release/snd -h
snd 0.1.0
a dns traffic generator

USAGE:
snd [OPTIONS] [FLAGS]

OPTIONS:
    -s, --server <server>                          the dns server for benchmark [default: 8.8.8.8]
    -p, --port <port>                              the dns server port number [default: 53]
    -d, --domain <domain>                          domain name for dns query [default: example.com]
    -t, --type <qty>                               dns query type [default: A]
    -T, --time <time>                              how long it will send until stop [default: 0]
    -q, --qps <qps>                                dns query per second [default: 10]
    -m, --max <max>                                max dns packets will be send [default: 100]
    -c, --client <client>                          concurrent clients numbers, set to 0 will replace with the number of cpu cores [default: 0]
    -f, --file <file>                              the dns query file, default using -d for single domain query [default: ""]
    -o, --output <file>                            format output report to stdout, .json or .yaml file [default: "stdout"]
    -I, --interval <second>                        output result interval for dns benchmark [default: 0]
        --edns-size <edns-size>                    set opt max EDNS buffer size [default: 1232]
        --protocol <protocol>                      the packet protocol for send dns request [default: UDP]
                                                   support protocols [UDP, TCP]
        --source-ip <source>                       set the source ip address [default: 0.0.0.0]
        --timeout <timeout>                        timeout for wait the packet arrive [default: 5]
        --bind-cpu <mode>                          bind worker to cpu [default: random]
                                                   option value ["random", "all", "0,1,2,3", "0,3"]

FLAGS:
        --debug                enable debug mode
        --disable-edns         disable EDNS
        --disable-rd           RD (recursion desired) bit in the query
        --enable-cd            CD (checking disabled) bit in the query
        --enable-dnssec        enable dnssec
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

##### DoH(main branch)

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


##### DNS over DOT(main)

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


#### 3. Load Test(mio-version branch) 


```
CPU       : Intel(R) Xeon(R) CPU E7-4820 v4 @ 2.00GHz 
            80 Core  
Memory    : 64 GB DDR4
DNS Server: knotd (Knot DNS), version 2.8.3
Network   : Ethernet controller: Intel Corporation Ethernet Controller X710 for 10GbE SFP+ (rev 01)
            Channel parameters for ens3f0::
            Pre-set maximums:
            RX:             0
            TX:             0
            Other:          1
            Combined:       80
            Current hardware settings:
            RX:             0
            TX:             0
            Other:          1
            Combined:       80

```


```
./snd -s 192.168.9.4 -d www.test.cn -t A -m 20000000 -q 0  -c 800 -I 1 --bind-cpu all 

------------   Report   --------------
      Total Cost: 10.823781656s
     Total Query: 20000000
        Question: A=20000000
  Total Response: 20000000
   Response Code: No Error=20000000
    Success Rate: 100.00%
     Average QPS: 1847783
     Min Latency: 42.739µs
     Max Latency: 126.659958ms
    Mean Latency: 275.238µs
     99% Latency: 720.31µs
     95% Latency: 381.252µs
     90% Latency: 326.202µs
     50% Latency: 233.171µs

```