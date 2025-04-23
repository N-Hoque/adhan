FROM debian:bookworm-slim

RUN apt update && \
    apt upgrade -y && \
    apt install -y \
        libasound2 \
        libasound2-dev

WORKDIR /app

CMD ["/app/salah"]
