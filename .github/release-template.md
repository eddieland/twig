# Release v${VERSION}

**Full Changelog**: https://github.com/eddieland/twig/compare/v${PREVIOUS_TAG}...v${VERSION}

## Download and Install

### Twig CLI

#### Ubuntu/Linux

```bash
# Download the release
curl -L -o twig-linux-x86_64.tar.gz https://github.com/eddieland/twig/releases/download/v${VERSION}/twig-linux-x86_64-v${VERSION}.tar.gz

# Extract the archive
tar -xzf twig-linux-x86_64.tar.gz

# Install to /usr/local/bin (requires sudo)
sudo cp twig /usr/local/bin/

# Make executable (if not already)
sudo chmod +x /usr/local/bin/twig

# Clean up downloaded files
rm twig-linux-x86_64.tar.gz twig
```

#### macOS

```bash
# Download the release
curl -L -o twig-macos-x86_64.tar.gz https://github.com/eddieland/twig/releases/download/v${VERSION}/twig-macos-x86_64-v${VERSION}.tar.gz

# Extract the archive
tar -xzf twig-macos-x86_64.tar.gz

# Install to /usr/local/bin (requires sudo)
sudo cp twig /usr/local/bin/

# Make executable (if not already)
sudo chmod +x /usr/local/bin/twig

# Clean up downloaded files
rm twig-macos-x86_64.tar.gz twig
```

### Twig Flow Plugin

#### Ubuntu/Linux

```bash
# Download the plugin release
curl -L -o twig-flow-linux-x86_64.tar.gz https://github.com/eddieland/twig/releases/download/v${VERSION}/twig-flow-linux-x86_64-v${VERSION}.tar.gz

# Extract the archive
tar -xzf twig-flow-linux-x86_64.tar.gz

# Install to /usr/local/bin (requires sudo)
sudo cp twig-flow /usr/local/bin/

# Make executable (if not already)
sudo chmod +x /usr/local/bin/twig-flow

# Clean up downloaded files
rm twig-flow-linux-x86_64.tar.gz twig-flow
```

#### macOS

```bash
# Download the plugin release
curl -L -o twig-flow-macos-x86_64.tar.gz https://github.com/eddieland/twig/releases/download/v${VERSION}/twig-flow-macos-x86_64-v${VERSION}.tar.gz

# Extract the archive
tar -xzf twig-flow-macos-x86_64.tar.gz

# Install to /usr/local/bin (requires sudo)
sudo cp twig-flow /usr/local/bin/

# Make executable (if not already)
sudo chmod +x /usr/local/bin/twig-flow

# Clean up downloaded files
rm twig-flow-macos-x86_64.tar.gz twig-flow
```

## Quick Install

### Twig CLI

#### Ubuntu/Linux

```bash
curl -L https://github.com/eddieland/twig/releases/download/v${VERSION}/twig-linux-x86_64-v${VERSION}.tar.gz | tar -xz && sudo cp twig /usr/local/bin/ && sudo chmod +x /usr/local/bin/twig && rm twig
```

#### macOS

```bash
curl -L https://github.com/eddieland/twig/releases/download/v${VERSION}/twig-macos-x86_64-v${VERSION}.tar.gz | tar -xz && sudo cp twig /usr/local/bin/ && sudo chmod +x /usr/local/bin/twig && rm twig
```

### Twig Flow Plugin

#### Ubuntu/Linux

```bash
curl -L https://github.com/eddieland/twig/releases/download/v${VERSION}/twig-flow-linux-x86_64-v${VERSION}.tar.gz | tar -xz && sudo cp twig-flow /usr/local/bin/ && sudo chmod +x /usr/local/bin/twig-flow && rm twig-flow
```

#### macOS

```bash
curl -L https://github.com/eddieland/twig/releases/download/v${VERSION}/twig-flow-macos-x86_64-v${VERSION}.tar.gz | tar -xz && sudo cp twig-flow /usr/local/bin/ && sudo chmod +x /usr/local/bin/twig-flow && rm twig-flow
```

## Verify Installation

```bash
# Check if twig is installed and accessible
which twig
# Check if twig-flow is installed and accessible
which twig-flow

# Check version
twig --version
twig-flow --help
```
