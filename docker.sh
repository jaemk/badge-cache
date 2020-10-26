#!/bin/bash

set -e

# update version
tag="$(git rev-parse HEAD | head -c 7 | awk '{ printf "%s", $0 }')"

echo "building images... latest, $tag "

docker build -t jaemk/badge-cache:$tag .
docker build -t jaemk/badge-cache:latest .

ports="-p 4000:4000"

# set envs from csv env var
if [[ -z "$ENVS" ]]; then
    envs="$envs"
else
    for e_str in $(echo $ENVS | tr "," "\n")
    do
        envs="-e $e_str $envs"
    done
fi

# set key-value pairs if there's an .env.local
if [[ -z "$ENVFILE" ]]; then
    if [ -d .env.local ]; then
        envfile="--env-file env.local"
    fi
else
    envfile="--env-file $ENVFILE"
fi


if [ "$1" = "run" ]; then
    echo "running..."
    set -x
    docker run --rm -it --init $ports $envs $envfile jaemk/badge-cache:latest
elif [ "$1" = "push" ]; then
    echo "pushing images..."
    set -x
    docker push jaemk/badge-cache:$tag
    docker push jaemk/badge-cache:latest
fi
