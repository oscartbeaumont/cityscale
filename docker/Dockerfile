FROM --platform=linux/amd64 mysql:8.3.0

EXPOSE 2489
EXPOSE 3306
VOLUME /data

COPY ./cityscale /usr/bin/cityscale

ENV DATA_DIR=/data/cityscale

ENTRYPOINT ["/usr/bin/cityscale"]