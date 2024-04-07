# build stage:
FROM debian:bookworm as build

WORKDIR /usr/src/app

RUN apt update && apt upgrade -y

RUN apt install -y \
    pkg-config \
    libssl-dev \
    build-essential \
    curl \
    libprotobuf-dev \
    protobuf-compiler

RUN apt update

# RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

RUN rustup update
RUN rustup default nightly

RUN rustup --version
RUN rustc --version
RUN cargo --version

RUN rustup component add rustfmt

COPY . .

RUN cargo build --release

# release stage:
FROM debian:bookworm-slim

RUN apt update && apt upgrade -y
RUN apt install -y \
    pkg-config \
    libssl-dev \
    curl

WORKDIR /usr/src/app

COPY --from=build /usr/src/app/target/release/rustix_bin ./rustix

CMD ["./rustix"]