## Integration: alpm

```
# /etc/pacman.conf
XferCommand=/usr/bin/repro-threshold transport alpm -O %o %u
```

## Integration: apt

```
#deb [arch=amd64] reproduced+http://deb.debian.org/debian unstable main

Types: deb
URIs: reproduced+http://deb.debian.org/debian
Suites: stable stable-updates
Components: main
Architectures: amd64
Signed-By: /usr/share/keyrings/debian-archive-keyring.gpg
```

