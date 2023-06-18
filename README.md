# RustGrok

Inspired by ngrok, a BLAZIN FAST ingress for your localhost apps.

# DevOps configuration

The server is deployed and redirected by Traefik, I query and update the Ingress that handle addresses being redirected.
It would be better to use an Ingress with a wildcard, but this need additional configuration for Traefik which I am not willing to make.

# Progress / Features

## PoC

- [x] Proxy tcp stream between two sockets
- [x] Reverse Proxy, checking the Host header from the http request
- [x] Enable clients to use a socket to request routing of the requests from a specific Host to this same socket.

## V1

- [x] Build client CLI to request the routing, and proxy the received traffic to the local port selected.
- [x] DevOps: deployment + traefik and dns config.

## V1.1

- [ ] Implement a multi stream version, to work with Browser request, that can keep-alive the connection
- [ ] Heartbeat and connection monitoring, to cleanup broken or disconnected routes/users.

# V1.x

- [ ] Better error handling


## V2

- [ ] Preserve connection between proxy server and the local server.
- [ ] TUI: display logs and details