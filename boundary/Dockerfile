# ADR-015 worker image: scratch + two fixed compiled entrypoints, no shell,
# no package manager, no writable layer content. Rebuild only through the
# pinned toolchain in the qualification lane; digest-pin at deploy time.
FROM scratch
COPY bin/capture /capture
COPY bin/exporter /exporter
ENTRYPOINT ["/capture"]
