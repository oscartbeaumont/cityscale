FROM --platform=linux/amd64 mysql:8.3.0

EXPOSE 3306

COPY ./cityscale /usr/bin/cityscale

ENTRYPOINT ["/usr/bin/cityscale"]