FROM ubuntu:20.04
# Update default packages
RUN apt-get update

# Get essential packages
RUN apt-get install -y build-essential curl


# Install additional networking and Python tools (if needed)
RUN apt-get install -y \
    iproute2 tcpdump python3 kmod net-tools bridge-utils iptables  && \
    apt-get install -y iputils-ping python3-pip && \
    rm -rf /var/lib/apt/lists/*

RUN apt-get update && \
    apt-get install -y  wireguard netcat-traditional


# Update package lists again (optional, can be combined with previous line)
RUN apt-get update

# Install Rustup with -y flag
RUN curl https://sh.rustup.rs -sSf | bash -s -- -y

# Set environment variable for Rustup path
ENV PATH="/root/.cargo/bin:${PATH}"


# Create TUN device (if needed)
RUN mkdir -p /dev/net && mknod /dev/net/tun c 10 200 && chmod 0666 /dev/net/tun

# Set default command (optional)
CMD ["tail", "-f", "/dev/null"]  # Keeps container running
