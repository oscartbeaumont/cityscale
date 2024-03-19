# TODO: Alpine based image


ARG bootstrap_version=22.6
ARG image="vitess/bootstrap:${bootstrap_version}-common"

FROM "${image}"

RUN apt-get update
RUN apt-get install -y sudo curl vim jq

RUN rm -rf /var/lib/apt/lists/*

# TODO: Drop vtadmin
RUN curl -fsSL https://deb.nodesource.com/setup_19.x |  bash - && apt-get install -y nodejs

RUN echo "source /vt/common/env.sh" >> /etc/bash.bashrc

# Allows some docker builds to disable CGO
ARG CGO_ENABLED=0

# TODO: Avoid building Vitess and instead download it
# Re-copy sources from working tree.
COPY --chown=vitess:vitess vitess /vt/src/vitess.io/vitess

# Build and install Vitess in a temporary output directory.
USER vitess

WORKDIR /vt/src/vitess.io/vitess
RUN make install PREFIX=/vt/install

ENV VTROOT /vt/src/vitess.io/vitess
ENV VTDATAROOT /vt/vtdataroot
ENV PATH $VTROOT/bin:$PATH
ENV PATH="/var/opt/etcd:${PATH}"

RUN mkdir /vt/local
COPY local /vt/local

# TODO: Drop vtadmin
# Copy the vtadmin web app to the correct location and npm install
COPY --chown=vitess:vitess vitess/web /web
RUN npm install /web/vtadmin
RUN /web/vtadmin/build.sh

RUN mkdir /vt/common
COPY common /vt/common

CMD cd /vt/local && ./101_initial_cluster.sh && /bin/bash
