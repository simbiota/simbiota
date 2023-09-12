#!/usr/bin/env sh

set -o errexit
set -o nounset

# Dropping privileges because downloading a file doesn't need root.
# The --inh-caps part is essentially the same as -all but on legacy raspbian
# `setpriv` returns 'setpriv: libcap-ng is too old for "all" caps' so
# we list all priviliges one-by-one with --list-caps.
setpriv --reuid=nobody --regid=nogroup --init-groups --inh-caps="$(setpriv --list-caps | xargs -I{} printf '-{},' | head -c-1)" sh -s <<- EOF
umask 0000
curl -s https://api.github.com/repos/simbiota/database-releases/releases/latest \
| jq '.assets[] | select(.name | match(".*-arm-.*")).browser_download_url' \
| tr -d '"\n' \
| xargs -0 -I{} curl -L -o /tmp/database.sdb '{}'
EOF

install -o root -g root -m 644 /tmp/database.sdb /var/lib/simbiota/database.sdb && rm /tmp/database.sdb