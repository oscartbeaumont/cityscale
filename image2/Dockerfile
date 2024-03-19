# TODO: We need `vtcombo`
# FROM "vitess/lite:latest"
# FROM "vitess/vitess:latest"
FROM "vitess/local:latest"

# TODO: Metadata

# TODO Scripts

WORKDIR /

COPY --chown=vitess:vitess topology /topology
COPY --chown=vitess:vitess vschema /vschema

COPY --chown=vitess:vitess run.sh /run.sh

CMD ["/run.sh"]