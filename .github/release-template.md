# Release v${VERSION}

**Full Changelog**: https://github.com/eddieland/twig/compare/v${PREVIOUS_TAG}...v${VERSION}

## Quick Install

### Twig CLI

**Linux:**

```bash
curl -L https://github.com/eddieland/twig/releases/download/v${VERSION}/twig-linux-x86_64-v${VERSION}.tar.gz | tar -xz && sudo cp twig /usr/local/bin/ && sudo chmod +x /usr/local/bin/twig && rm twig
```

**macOS:**

```bash
curl -L https://github.com/eddieland/twig/releases/download/v${VERSION}/twig-macos-x86_64-v${VERSION}.tar.gz | tar -xz && sudo cp twig /usr/local/bin/ && sudo chmod +x /usr/local/bin/twig && rm twig
```

Or self-update an existing installation:

```bash
twig self update
```

<details>
<summary><strong>Twig Flow Plugin</strong></summary>

**Linux:**

```bash
curl -L https://github.com/eddieland/twig/releases/download/v${VERSION}/twig-flow-linux-x86_64-v${VERSION}.tar.gz | tar -xz && sudo cp twig-flow /usr/local/bin/ && sudo chmod +x /usr/local/bin/twig-flow && rm twig-flow
```

**macOS:**

```bash
curl -L https://github.com/eddieland/twig/releases/download/v${VERSION}/twig-flow-macos-x86_64-v${VERSION}.tar.gz | tar -xz && sudo cp twig-flow /usr/local/bin/ && sudo chmod +x /usr/local/bin/twig-flow && rm twig-flow
```

Or: `twig self update flow`

</details>

<details>
<summary><strong>Twig Prune Plugin</strong></summary>

**Linux:**

```bash
curl -L https://github.com/eddieland/twig/releases/download/v${VERSION}/twig-prune-linux-x86_64-v${VERSION}.tar.gz | tar -xz && sudo cp twig-prune /usr/local/bin/ && sudo chmod +x /usr/local/bin/twig-prune && rm twig-prune
```

**macOS:**

```bash
curl -L https://github.com/eddieland/twig/releases/download/v${VERSION}/twig-prune-macos-x86_64-v${VERSION}.tar.gz | tar -xz && sudo cp twig-prune /usr/local/bin/ && sudo chmod +x /usr/local/bin/twig-prune && rm twig-prune
```

Or: `twig self update prune`

</details>

<details>
<summary><strong>Twig MCP Server</strong></summary>

**Linux:**

```bash
curl -L https://github.com/eddieland/twig/releases/download/v${VERSION}/twig-mcp-linux-x86_64-v${VERSION}.tar.gz | tar -xz && sudo cp twig-mcp /usr/local/bin/ && sudo chmod +x /usr/local/bin/twig-mcp && rm twig-mcp
```

**macOS:**

```bash
curl -L https://github.com/eddieland/twig/releases/download/v${VERSION}/twig-mcp-macos-x86_64-v${VERSION}.tar.gz | tar -xz && sudo cp twig-mcp /usr/local/bin/ && sudo chmod +x /usr/local/bin/twig-mcp && rm twig-mcp
```

Or: `twig self update mcp`

</details>

<details>
<summary><strong>Manual Install (step-by-step)</strong></summary>

### Twig CLI

**Linux:**

```bash
curl -L -o twig-linux-x86_64.tar.gz https://github.com/eddieland/twig/releases/download/v${VERSION}/twig-linux-x86_64-v${VERSION}.tar.gz
tar -xzf twig-linux-x86_64.tar.gz
sudo cp twig /usr/local/bin/
sudo chmod +x /usr/local/bin/twig
rm twig-linux-x86_64.tar.gz twig
```

**macOS:**

```bash
curl -L -o twig-macos-x86_64.tar.gz https://github.com/eddieland/twig/releases/download/v${VERSION}/twig-macos-x86_64-v${VERSION}.tar.gz
tar -xzf twig-macos-x86_64.tar.gz
sudo cp twig /usr/local/bin/
sudo chmod +x /usr/local/bin/twig
rm twig-macos-x86_64.tar.gz twig
```

### Twig Flow Plugin

**Linux:**

```bash
curl -L -o twig-flow-linux-x86_64.tar.gz https://github.com/eddieland/twig/releases/download/v${VERSION}/twig-flow-linux-x86_64-v${VERSION}.tar.gz
tar -xzf twig-flow-linux-x86_64.tar.gz
sudo cp twig-flow /usr/local/bin/
sudo chmod +x /usr/local/bin/twig-flow
rm twig-flow-linux-x86_64.tar.gz twig-flow
```

**macOS:**

```bash
curl -L -o twig-flow-macos-x86_64.tar.gz https://github.com/eddieland/twig/releases/download/v${VERSION}/twig-flow-macos-x86_64-v${VERSION}.tar.gz
tar -xzf twig-flow-macos-x86_64.tar.gz
sudo cp twig-flow /usr/local/bin/
sudo chmod +x /usr/local/bin/twig-flow
rm twig-flow-macos-x86_64.tar.gz twig-flow
```

### Twig Prune Plugin

**Linux:**

```bash
curl -L -o twig-prune-linux-x86_64.tar.gz https://github.com/eddieland/twig/releases/download/v${VERSION}/twig-prune-linux-x86_64-v${VERSION}.tar.gz
tar -xzf twig-prune-linux-x86_64.tar.gz
sudo cp twig-prune /usr/local/bin/
sudo chmod +x /usr/local/bin/twig-prune
rm twig-prune-linux-x86_64.tar.gz twig-prune
```

**macOS:**

```bash
curl -L -o twig-prune-macos-x86_64.tar.gz https://github.com/eddieland/twig/releases/download/v${VERSION}/twig-prune-macos-x86_64-v${VERSION}.tar.gz
tar -xzf twig-prune-macos-x86_64.tar.gz
sudo cp twig-prune /usr/local/bin/
sudo chmod +x /usr/local/bin/twig-prune
rm twig-prune-macos-x86_64.tar.gz twig-prune
```

### Twig MCP Server

**Linux:**

```bash
curl -L -o twig-mcp-linux-x86_64.tar.gz https://github.com/eddieland/twig/releases/download/v${VERSION}/twig-mcp-linux-x86_64-v${VERSION}.tar.gz
tar -xzf twig-mcp-linux-x86_64.tar.gz
sudo cp twig-mcp /usr/local/bin/
sudo chmod +x /usr/local/bin/twig-mcp
rm twig-mcp-linux-x86_64.tar.gz twig-mcp
```

**macOS:**

```bash
curl -L -o twig-mcp-macos-x86_64.tar.gz https://github.com/eddieland/twig/releases/download/v${VERSION}/twig-mcp-macos-x86_64-v${VERSION}.tar.gz
tar -xzf twig-mcp-macos-x86_64.tar.gz
sudo cp twig-mcp /usr/local/bin/
sudo chmod +x /usr/local/bin/twig-mcp
rm twig-mcp-macos-x86_64.tar.gz twig-mcp
```

</details>

<details>
<summary><strong>Verify Installation</strong></summary>

```bash
# Check if binaries are installed and accessible
which twig
which twig-flow
which twig-prune
which twig-mcp

# Check versions
twig --version
twig-flow --version
twig-prune --version
twig-mcp --version
```

</details>
