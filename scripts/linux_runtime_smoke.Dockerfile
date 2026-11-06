FROM ubuntu:24.04

RUN apt-get update && apt-get install --no-install-recommends -y \
    ca-certificates \
    dbus-x11 \
    fonts-dejavu-core \
    imagemagick \
    libasound2t64 \
    libegl1 \
    libfontconfig1 \
    libfreetype6 \
    libgles2 \
    libglib2.0-0t64 \
    libstdc++6 \
    libvulkan1 \
    libwayland-client0 \
    libwayland-cursor0 \
    libwayland-egl1 \
    libx11-6 \
    libxcb-randr0 \
    libxcb-shape0 \
    libxcb-xfixes0 \
    libxcb-xinput0 \
    libxkbcommon-x11-0 \
    libxkbcommon0 \
    mesa-vulkan-drivers \
    python3 \
    python3-pil \
    x11-apps \
    xauth \
    xdg-utils \
    xvfb \
    && rm -rf /var/lib/apt/lists/*

ENV LIBGL_ALWAYS_SOFTWARE=1
WORKDIR /tmp
