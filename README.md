# **Debot** 🚀  

**Debot** is a decentralized system leveraging **libp2p** for networking, **Raft** for consensus, and **Telegram bots** for interactive communication. It enables **fault-tolerant messaging** and **state synchronization** across distributed nodes, making it ideal for **scalable and resilient applications**.  

🔗 **GitHub Repository**: [scriptkid23/debot](https://github.com/scriptkid23/debot)  

---

## **📌 Features**  

✅ **Decentralized Networking**: Built using `libp2p` for peer-to-peer communication.  
✅ **Consensus Algorithm**: Uses `Raft` to achieve distributed consensus among nodes.  
✅ **Interactive Communication**: Supports `Telegram Bots` for real-time user interaction.  
✅ **Scalable & Fault-Tolerant**: Ensures system reliability in distributed environments.  

---

## **🛠️ Setup & Installation**  

### **1️⃣ Prerequisites**  

Ensure you have the following installed on your system:  

- **Rust** (Latest stable version)  
- **Cargo** (Rust’s package manager)  
- **Git** (For version control)  

### **2️⃣ Install Rust & Cargo**  

If Rust is not installed, run:  

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env  # Ensure Rust is available in your shell
```

Verify installation:  

```bash
rustc --version  # Should output Rust version
cargo --version  # Should output Cargo version
```

### **3️⃣ Clone the Repository**  

```bash
git clone https://github.com/scriptkid23/debot.git
cd debot
```

### **4️⃣ Build the Project**  

```bash
cargo build --release
```

### **5️⃣ Run the Project**  

```bash
cargo run
```

or in release mode (optimized for performance):  

```bash
cargo run --release
```

### **6️⃣ Run Tests**  

```bash
cargo test
```

---

## **🚀 Running Debot in Production**  

To deploy Debot efficiently, run it in **release mode** to optimize performance:  

```bash
cargo build --release
./target/release/debot
```

To monitor logs, run:  

```bash
RUST_LOG=info ./target/release/debot
```

For background execution, use:  

```bash
nohup ./target/release/debot &> debot.log &
```

---

## **📚 References**  

- [Raft Consensus Algorithm](https://raft.github.io/)  
- [libp2p Networking](https://libp2p.io/)  
- [Simple Raft RS Implementation](https://github.com/simple-raft-rs/raft-rs/blob/master/src/core.rs)  

---

## **❓ Need Help?**  

- **Check the Documentation**: [Debot Docs](https://docs.debot.io)  
- **Join the Community**: [Debot Forum](https://community.debot.io)  
- **Discord Support**: [Join Discord](https://discord.gg/debot)  
- **Email Us**: [support@1hoodlabs.com](mailto:support@1hoodlabs.com)  

---

## **👏 Thank You for Using Debot!**  

We appreciate your interest in **Debot**. Your contributions and feedback help us improve. **Happy coding! 💻✨**  
