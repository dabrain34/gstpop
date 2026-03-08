# Multi-arch Dockerfile for gst-pop
# Supports linux/amd64 (prebuilt binary) and linux/arm64 (built from source)
#
# Build:
#   docker buildx build --platform linux/amd64,linux/arm64 -t gst-pop .
#
# Run:
#   docker run -d -p 9000:9000 --name gst-pop gst-pop

# ============================================================
# Stage 1: Builder - fetch or compile gst-pop binaries
# ============================================================
FROM fedora:42 AS builder

ARG TARGETARCH
ARG GST_POP_VERSION=0.4.2

RUN dnf install -y curl && dnf clean all

# --- amd64: download prebuilt binaries from GitHub release ---
RUN if [ "$TARGETARCH" = "amd64" ]; then \
      dnf install -y unzip && dnf clean all \
      && curl -fsSL \
        "https://github.com/dabrain34/gstpop/releases/download/v${GST_POP_VERSION}/gst-pop-v${GST_POP_VERSION}-linux-x86_64.zip" \
        -o /tmp/gst-pop.zip \
      && mkdir -p /tmp/staging \
      && unzip /tmp/gst-pop.zip -d /tmp/staging \
      && rm /tmp/gst-pop.zip; \
    fi

# --- arm64: build from source tarball ---
RUN if [ "$TARGETARCH" = "arm64" ]; then \
      dnf install -y \
        gcc \
        gstreamer1-devel \
        gstreamer1-plugins-base-devel \
        rust \
        cargo \
        tar \
        gzip \
      && dnf clean all \
      && curl -fsSL \
        "https://github.com/dabrain34/gstpop/releases/download/v${GST_POP_VERSION}/gst-pop-v${GST_POP_VERSION}-source.tar.gz" \
        -o /tmp/source.tar.gz \
      && mkdir -p /tmp/gst-pop-src \
      && tar -xzf /tmp/source.tar.gz -C /tmp/gst-pop-src --strip-components=1 \
      && cd /tmp/gst-pop-src \
      && cargo build --release --workspace \
      && mkdir -p /tmp/staging \
      && cp target/release/gst-pop /tmp/staging/ \
      && cp target/release/gst-popctl /tmp/staging/ \
      && rm -rf /tmp/gst-pop-src /tmp/source.tar.gz; \
    fi

# Fail early if the architecture produced no binaries
RUN test -f /tmp/staging/gst-pop || \
    (echo "ERROR: Unsupported architecture: ${TARGETARCH}" && exit 1)

# ============================================================
# Stage 2: Runtime image with GStreamer
# ============================================================
FROM fedora:42

ARG GST_POP_VERSION=0.4.2

LABEL maintainer="Stephane Cerveau <scerveau@igalia.com>"
LABEL description="gst-pop v${GST_POP_VERSION} - GStreamer pipeline management daemon"
LABEL version="${GST_POP_VERSION}"

# --- Install GStreamer with maximum plugin coverage ---
RUN dnf -y install \
      gstreamer1 \
      gstreamer1-plugins-base \
      gstreamer1-plugins-base-tools \
      gstreamer1-plugins-good \
      gstreamer1-plugins-good-extras \
      gstreamer1-plugins-bad-free \
      gstreamer1-plugins-bad-free-extras \
      gstreamer1-plugins-ugly-free \
      gstreamer1-plugin-libav \
      gstreamer1-plugin-openh264 \
      gstreamer1-vaapi \
      bash \
    && dnf clean all

# --- Install gst-pop binaries from builder stage ---
COPY --from=builder --chmod=755 /tmp/staging/gst-pop /usr/bin/gst-pop
COPY --from=builder --chmod=755 /tmp/staging/gst-popctl /usr/bin/gst-popctl

# --- Create busybox-style symlinks ---
RUN ln -sf gst-pop /usr/bin/gst-popd && \
    ln -sf gst-pop /usr/bin/gst-pop-launch && \
    ln -sf gst-pop /usr/bin/gst-pop-inspect && \
    ln -sf gst-pop /usr/bin/gst-pop-discover && \
    ln -sf gst-pop /usr/bin/gst-pop-play

# --- Create non-root user ---
RUN useradd -r -s /sbin/nologin -d /nonexistent gstpop

USER gstpop

EXPOSE 9000

HEALTHCHECK --interval=30s --timeout=5s --retries=3 \
  CMD bash -c '</dev/tcp/localhost/9000' || exit 1

ENTRYPOINT ["/usr/bin/gst-popd"]
CMD ["--bind", "0.0.0.0", "--no-dbus"]
