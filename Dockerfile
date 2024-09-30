FROM ubuntu:latest

WORKDIR /app
COPY rand-api /app/rand-api
RUN chmod +x /app/rand-api

EXPOSE 4242
CMD ["./rand-api"]
# ENTRYPOINT ["tail", "-f", "/dev/null"]
