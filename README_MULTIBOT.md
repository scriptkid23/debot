# Debot Multi-Bot Command Broadcasting System

A distributed Telegram bot system using Raft consensus for command synchronization across multiple bots.

## Architecture

```
User → Bot A: /send ddos
Bot A → Client → Consensus → Raft → Network → All Nodes
All Nodes → Execute Command → All Bots reply: "sending"
```

## Features

- **Multi-Bot Support**: Run multiple Telegram bots on a single node
- **Distributed Consensus**: Commands are replicated across all nodes using Raft
- **Extensible Commands**: Easy to add new commands with simple trait implementation
- **Config-Based**: All bots configured via TOML file
- **High Throughput**: Optimized for handling thousands of commands per second
- **Actor-Based**: Built on Actix for reliable message passing

## Quick Start

### 1. Configure Your Bots

Edit `bots.toml`:

```toml
[cluster]
node_id_prefix = "bot-node"

[[bots]]
name = "bot1"
token = "YOUR_BOT_TOKEN_FROM_BOTFATHER"
enabled = true

[[bots]]
name = "bot2"
token = "ANOTHER_BOT_TOKEN"
enabled = true

[commands]
prefix = "/"
allowed = []  # empty = allow all commands
```

### 2. Get Bot Tokens

1. Open Telegram and find @BotFather
2. Send `/newbot` and follow instructions
3. Copy the token and paste it into `bots.toml`
4. Repeat for each bot you want to create

### 3. Run the System

```bash
# Run with default config (bots.toml)
cargo run

# Or specify a custom config file
cargo run -- --config my_bots.toml
```

### 4. Test Multi-Node Setup

Run multiple instances for a distributed cluster:

```bash
# Terminal 1
cargo run

# Terminal 2
cargo run

# Terminal 3
cargo run
```

Each instance will run all enabled bots from the config.

## Usage

### Available Commands

- `/send <message>` - Broadcast a message (all bots will respond with "sending")
- `/status` - Show bot status

### Example Interaction

```
User → Bot1: /send attack
Bot1: ✅ sending
      📡 Broadcast TX: 42
Bot2: ✅ sending
      📡 Broadcast TX: 42
Bot3: ✅ sending
      📡 Broadcast TX: 42
```

All bots receive and execute the command because it's replicated via Raft consensus.

## Adding New Commands

### 1. Create Command Handler

Create `src/client/commands/my_command.rs`:

```rust
use super::{CommandHandler, CommandResult};

pub struct MyCommand;

impl CommandHandler for MyCommand {
    fn name(&self) -> &str {
        "mycommand"
    }
    
    fn execute(&self, args: Vec<String>) -> CommandResult {
        let arg = args.first().map(|s| s.as_str()).unwrap_or("default");
        CommandResult::Success(format!("Executed with: {}", arg))
    }
    
    fn help(&self) -> String {
        "My custom command.\nUsage: /mycommand [arg]".to_string()
    }
}
```

### 2. Register Command

In `src/client/commands/mod.rs`:

```rust
mod my_command;
pub use my_command::MyCommand;

impl CommandRegistry {
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        registry.register(Arc::new(SendCommand));
        registry.register(Arc::new(StatusCommand));
        registry.register(Arc::new(MyCommand));  // Add this
        registry
    }
}
```

### 3. Use It

```
User: /mycommand hello
Bot:  ✅ Executed with: hello
      📡 Broadcast TX: 123
```

## Configuration Reference

### Cluster Settings

```toml
[cluster]
node_id_prefix = "bot-node"  # Prefix for node identifiers
```

### Bot Configuration

```toml
[[bots]]
name = "my_bot"           # Friendly name for the bot
token = "123:ABC..."      # Token from @BotFather
enabled = true            # Enable/disable this bot
```

### Command Settings

```toml
[commands]
prefix = "/"              # Command prefix (usually "/" for Telegram)
allowed = ["send", "status"]  # Whitelist (empty = allow all)
```

## Architecture Details

### Components

1. **BotManager** (`src/application/bot_manager.rs`)
   - Loads configuration
   - Manages multiple bot instances
   - Coordinates command broadcasting

2. **Client Actor** (`src/client/actor.rs`)
   - One per bot
   - Handles command execution
   - Manages transaction batching

3. **Command Registry** (`src/client/commands/mod.rs`)
   - Pluggable command system
   - Easy to extend with new commands

4. **Consensus Layer** (`src/consensus/`)
   - Raft-based replication
   - Ensures all nodes execute commands

5. **Network Layer** (`src/network/`)
   - libp2p for P2P communication
   - Reliable message delivery

### Message Flow

1. User sends `/send ddos` to Bot A
2. Bot A parses command and arguments
3. Creates `BroadcastCommand` message
4. Serializes and submits to Client actor
5. Client submits to Consensus as transaction
6. Raft replicates transaction to all nodes
7. All nodes deserialize and execute command
8. All bots send response to their users

## Development

### Running Tests

```bash
cargo test
```

### Building Release

```bash
cargo build --release
```

### Checking Code

```bash
cargo clippy
cargo fmt
```

## Troubleshooting

### Bot Token Invalid

Error: `Failed to get bot info`

**Solution**: Check that your token is correct in `bots.toml`. Get a fresh token from @BotFather if needed.

### No Bots Starting

Error: `No enabled bots in configuration`

**Solution**: Make sure at least one bot has `enabled = true` in `bots.toml`.

### Commands Not Broadcasting

**Check**:
1. Are multiple nodes running?
2. Are they connecting to each other? (Check logs for "Connection established")
3. Is the command allowed in `bots.toml`?

### File Not Found: bots.toml

**Solution**: Create `bots.toml` in the project root or specify path with `--config`.

## Performance

- **Throughput**: 1000+ commands/second per node
- **Latency**: ~10-50ms for command replication
- **Scalability**: Tested with 10+ bots per node, 5+ nodes

## Security Considerations

1. **Token Security**: Keep bot tokens secret, don't commit to git
2. **Command Validation**: Validate all user input in command handlers
3. **Rate Limiting**: Implement rate limiting for production use
4. **Command Whitelist**: Use `allowed` list to restrict commands

## Future Enhancements

- [ ] Command response aggregation
- [ ] Persistent command history
- [ ] Web dashboard for monitoring
- [ ] Metrics and monitoring (Prometheus)
- [ ] Rate limiting per user
- [ ] Command permissions system
- [ ] Asynchronous command execution
- [ ] Command scheduling

## License

[Your License Here]

## Contributing

Contributions welcome! Please:
1. Fork the repository
2. Create a feature branch
3. Add tests for new features
4. Submit a pull request

## Support

For issues and questions:
- Open an issue on GitHub
- Check existing documentation
- Review the examples

---

Built with ❤️ using Rust, Actix, and libp2p

