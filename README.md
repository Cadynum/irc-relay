# IRC relay bot
The bot creates a UNIX socket and listens to it. It then connects to one irc network and one channel. Any text sent to the UNIX socket will be relayed to the irc channel.  
* Author: Christoffer Öjeling
* License: AGPL-3  


### Configuration
It is configured in JSON. By default it reads `/etc/irc-relay.json`, but the executable can be given an argument to another file path.

### Example
The example requires openbsd-netcat, as gnu netcat cannot communicate with UNIX sockets.
``` 
./irc-relay irc-relay.json
echo -n Test message | nc -U /path/to/unix/socket
```

### Build
Currently targeting Rust 1.0.0beta.3
```
cargo build
```

### Missing features
It does not currently handle network kicks/nick collisions/probably other stuff...
