# repro-threshold

Threshold-based reproducible builds client using your trusted rebuilders

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

## License

`Apache-2.0 OR MIT-0`
