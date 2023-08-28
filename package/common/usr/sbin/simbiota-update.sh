#!/usr/bin/env sh

set -o errexit
set -o nounset

setpriv --reuid=nobody --regid=nogroup --init-groups --inh-caps=-all sh -s <<- EOF
umask 0000
curl -s https://api.github.com/repos/simbiota/database-releases/releases/latest \
| jq '.assets[] | select(.name | match(".*-arm-.*")).browser_download_url' \
| tr -d '"\n' \
| xargs -0 -I{} curl -L -o /tmp/database.sdb '{}'
EOF

install -o root -g root -m 644 /tmp/database.sdb /var/lib/simbiota/database.sdb && rm /tmp/database.sdb