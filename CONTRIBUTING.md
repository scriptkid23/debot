# **Contributing to Debot** 🚀  

Thank you for your interest in contributing to **Debot**, an open-source project by **1HoodLabs**! Your contributions—whether in code, documentation, or discussions—help make **Debot** better for everyone.  

Please read our **contribution guidelines** carefully before submitting issues or pull requests (PRs).  

---

## **📜 Contribution Guidelines**  

For general contribution best practices, see the **[1HoodLabs Open Source Contribution Guide](https://1hoodlabs.com/contribute)**.  

---

## **📖 Documentation**  

- **If your PR affects documentation**, make sure to include relevant updates in the **docs site**.  
- **Code-level documentation expectations**:  
  - **100% documentation coverage** for PRs affecting public APIs.  
  - **Include links to related documentation pages**, if applicable.  

---

## **🎨 Assets Compilation**  

If your contribution involves modifying **CSS, JS, SVG**, or other assets, follow these guidelines:  

1. **Ensure all assets are minified and optimized**.  
2. **Follow project-specific naming conventions**.  
3. **Run the asset build process**:  

   ```bash
   npm run build  # or another command specified in the README
   ```

---

## **🛠️ Environment Setup**  

To set up **Debot** locally, follow the installation steps in the **[README](README.md#installation)**.  

### **Additional Setup Notes**  

- Ensure that you have **Rust installed**:  

  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  ```

- If using **Docker**, refer to `docker-compose.yml` for containerized setup.  

---

## **🧪 Testing**  

### **✅ Adding Tests**  

- All PRs must include **unit tests** and **integration tests** if applicable.  
- Test files should be structured using the **Rust test module system**.  
- Use `#[test]` annotations and follow best practices for **test-driven development (TDD)**.  

### **🏃 Running Tests**  

Run the full test suite using:  

```bash
cargo test
```

For integration tests, use:  

```bash
cargo test --test integration_tests
```

Additionally, ensure you **run manual tests** if necessary.

---

## **🛠️ Code Quality & Linting**  

Before committing, **run code quality tools**:  

```bash
cargo fmt  # Format the code
cargo clippy  # Lint the code for errors & warnings
```

**Make sure there are no warnings or errors before submitting a PR**.

---

## **🔄 Continuous Integration (CI) Information**  

All PRs go through **CI checks** before merging.  

✅ **Checks include:**  

- Formatting (`cargo fmt --check`)  
- Linting (`cargo clippy --deny warnings`)  
- Unit and Integration Tests (`cargo test`)  
- Security Audit (`cargo audit`)  

**❌ Your PR will not be merged if it fails CI.**  

---

## **📌 Repo-Specific PR Guidelines**  

- **One feature/fix per PR**: Keep changes focused.  
- **Follow commit message conventions**:  

  ```bash
  feat(auth): Added OAuth2 support
  fix(network): Resolved timeout issue
  docs(readme): Updated installation guide
  ```

- **Ensure your PR includes tests and documentation updates (if needed).**  
- **Wait for approval before merging**.  

---

## **📧 Need Help?**  

- Check the **[Debot Documentation](https://docs.debot.io)**.  
- Ask in our **[Community Forum](https://community.debot.io)**.  
- Join our **[Discord](https://discord.gg/debot)**.  
- Email us at **[support@1hoodlabs.com](mailto:support@1hoodlabs.com)**.  

---

## **🚀 Thank You for Contributing!**  

We appreciate your time and effort in making **Debot** better. **Happy coding! 💻✨**
