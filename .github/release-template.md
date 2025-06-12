# Release v${VERSION}

**Full Changelog**: https://github.com/eddieland/twig/compare/v${PREVIOUS_TAG}...v${VERSION}

## Download and Install

### Ubuntu/Linux

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

### macOS

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

## Quick Install

### Ubuntu/Linux

```bash
curl -L https://github.com/eddieland/twig/releases/download/v${VERSION}/twig-linux-x86_64-v${VERSION}.tar.gz | tar -xz && sudo cp twig /usr/local/bin/ && sudo chmod +x /usr/local/bin/twig && rm twig
```

### macOS

```bash
curl -L https://github.com/eddieland/twig/releases/download/v${VERSION}/twig-macos-x86_64-v${VERSION}.tar.gz | tar -xz && sudo cp twig /usr/local/bin/ && sudo chmod +x /usr/local/bin/twig && rm twig
```

## Verify Installation

```bash
# Check if twig is installed and accessible
which twig

# Check version
twig --version
```
