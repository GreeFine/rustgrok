# RustGrok

Inspired by ngrok, a BLAZIN FAST ingress for your localhost apps.

# Progress / Features

## PoC

- [x] Proxy tcp stream between two sockets
- [x] Reverse Proxy, checking the Host header from the http request
- [x] Enable clients to use a socket to request routing of the requests from a specific Host to this same socket.

## V1

- [ ] Build client CLI to request the routing, and proxy the received traffic to the local port selected.
- [ ] Better error handling, ...
- [ ] DevOps: deployment + traefik and dns config.