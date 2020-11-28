# ip-update

## What is ip-update

`ip-update` does:
- fetch device's wan ip address
- publish new ip to NATS server or output to stdout
  - conpare current ip address, then if no update, no publish happens

## How to use

### If you want to publish your result to NATS

#### With default subject name `new_ip`

```
./ip-update -s publish.server.com -D mydomain.com -d 192.168.0.1:161
```

#### With custom subject name `foobar`

```
./ip-update -s publish.server.com -D mydomain.com -d 192.168.0.1:161 -S foobar
```

### If you want to ouput to stdout

```
./ip-update -s publish.server.com -D mydomain.com -d 192.168.0.1:161 -o
```
