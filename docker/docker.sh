DOCKER_IMAGE="kazee/ladder"
CONTAINER_NAME="ladder"

docker ps | grep ${CONTAINER_NAME} > /dev/null 2>&1

if [ $? -ne 0 ]; then
    # remove dead container
    docker rm ${CONTAINER_NAME} > /dev/null 2>&1

    # run new container
    docker run -itd \
        --net=host \
        --volume ${HOME}/.local:/root/.local  \
        --volume `pwd`:/chain  \
        --name ${CONTAINER_NAME} \
        ${DOCKER_IMAGE}

    sleep 3
fi

# enter container
docker exec -it ${CONTAINER_NAME} /bin/bash
