#!/bin/bash

set -e

# update version
tag="$(git rev-parse HEAD | head -c 7 | awk '{ printf "%s", $0 }')"
reg=docker.jaemk.me

echo "building images... latest, $tag "

docker build -t $reg/badge-cache:$tag .
docker build -t $reg/badge-cache:latest .

ports="-p 3003:3003"

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

root=$(git rev-parse --show-toplevel)

if [ "$1" = "run" ]; then
    echo "running..."
    set -x
    docker run --rm -it --init $ports $envs $envfile -v $root/cache_dir:/badge-cache/cache_dir $reg/badge-cache:latest
elif [ "$1" = "push" ]; then
    echo "pushing images..."
    set -x
    docker push $reg/badge-cache:$tag
    docker push $reg/badge-cache:latest
fi
