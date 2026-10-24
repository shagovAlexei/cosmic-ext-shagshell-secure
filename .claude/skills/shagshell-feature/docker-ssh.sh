#!/usr/bin/env bash
# Password-login ssh servers in docker, for checking askpass / host key / ProxyJump by hand.
#
#   docker-ssh.sh up      127.0.0.1:2201  (user test, password test) — the jump host
#                         `sshtarget`      same user / password, no published port: only the jump host
#                                          reaches it (ssh -J test@127.0.0.1:2201 test@sshtarget)
#   docker-ssh.sh down
# Sessions to try (Session Options): host 127.0.0.1 port 2201 user test; and host sshtarget
# user test with «More ssh options» `-J test@127.0.0.1:2201`.
set -euo pipefail
IMG=shagshell-test-sshd NET=shagshell-test
case ${1:-} in
up)
    if ! docker image inspect $IMG >/dev/null 2>&1; then
        docker build -q -t $IMG - <<'DOCKERFILE'
FROM alpine:3.20
RUN apk add --no-cache openssh bash && ssh-keygen -A \
 && adduser -D -s /bin/bash test && echo 'test:test' | chpasswd \
 && sed -i 's/^#\?AllowTcpForwarding.*/AllowTcpForwarding yes/' /etc/ssh/sshd_config
CMD ["/usr/sbin/sshd", "-D", "-e"]
DOCKERFILE
    fi
    docker network inspect $NET >/dev/null 2>&1 || docker network create $NET >/dev/null
    docker rm -f sshjump sshtarget >/dev/null 2>&1 || true
    docker run -d --name sshjump --network $NET -p 127.0.0.1:2201:22 $IMG >/dev/null
    docker run -d --name sshtarget --network $NET $IMG >/dev/null
    sleep 1; echo "up: 127.0.0.1:2201 (test/test), sshtarget via it" ;;
down) docker rm -f sshjump sshtarget >/dev/null 2>&1 || true; docker network rm $NET >/dev/null 2>&1 || true ;;
*) sed -n '2,9p' "$0"; exit 1 ;;
esac
