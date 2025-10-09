CANTON_VERSION=3.4.0-snapshot.20250617.16217.0.vbdf62919

docker run --rm -it \
  --volume "$PWD/config:/canton/config" \
  --name console-sandbox \
  digitalasset-docker.jfrog.io/canton-enterprise:${CANTON_VERSION} \
  --no-tty \
  -c /canton/config/remote-participant.conf