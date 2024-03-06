## Service configuration files

- `miden-faucet.service`: simple configuration to setup a faucet service monitored via systemd.
- `miden-node.service`: simple configuration to setup full node service monitored via systemd.

Install the above files to your system's unit directory (e.g.
`/etc/systemd/system/`), and run the following commands to start the service:

```sh
systemctl daemon-reload                 # ask the systemd to load the new configuration files
systemctl enable --now miden-node       # enable and start the node
systemctl enable --now miden-faucet     # enable and start the faucet
```
