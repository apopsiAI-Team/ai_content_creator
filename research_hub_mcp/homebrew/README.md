# Homebrew Formula for rust-sci-hub-mcp

This directory contains the Homebrew formula and related files for distributing rust-sci-hub-mcp via Homebrew.

## Files

- `rust-sci-hub-mcp.rb` - Main Homebrew formula
- `update_formula.sh` - Script to update formula with new versions
- `release.toml` - Release configuration for automation
- `checksums.txt` - Historical checksums for releases
- `README.md` - This file

## Formula Overview

The Homebrew formula provides:

- **Binary Installation**: Compiles and installs the rust-sci-hub-mcp binary
- **LaunchAgent Integration**: Installs and configures macOS LaunchAgent
- **Configuration Management**: Sets up default configuration files
- **Service Integration**: Works with `brew services` for service management
- **Multi-Architecture Support**: Builds for both Intel and Apple Silicon Macs

## Usage

### For End Users

```bash
# Install
brew install yourusername/rust-sci-hub-mcp/rust-sci-hub-mcp

# Start service
brew services start rust-sci-hub-mcp

# Check status
brew services list | grep rust-sci-hub-mcp
```

### For Maintainers

#### Updating the Formula

When releasing a new version:

```bash
# Update formula automatically
./homebrew/update_formula.sh 1.2.3

# Or manually specify URL and checksum
./homebrew/update_formula.sh 1.2.3 --url https://github.com/user/repo/archive/v1.2.3.tar.gz --sha256 abc123...

# Test the updated formula
brew install --build-from-source homebrew/rust-sci-hub-mcp.rb
brew test rust-sci-hub-mcp
```

#### Formula Validation

```bash
# Audit the formula for compliance
brew audit --strict homebrew/rust-sci-hub-mcp.rb

# Style check
brew style homebrew/rust-sci-hub-mcp.rb

# Install test
brew install --build-from-source homebrew/rust-sci-hub-mcp.rb
```

## Formula Structure

### Dependencies

**Build Dependencies:**
- `rust` - Rust toolchain for compilation
- `pkg-config` - For finding system libraries

**Runtime Dependencies:**
- `openssl@3` - TLS/SSL support
- `curl` - HTTP client library

### Installation Process

1. **Build Phase**: Compiles the Rust binary using `cargo install`
2. **Installation Phase**: 
   - Installs binary to appropriate location
   - Sets up LaunchAgent plist file
   - Creates necessary directories
   - Installs default configuration
3. **Post-Install Phase**:
   - Creates user-specific directories
   - Copies configuration to user directory
   - Sets up LaunchAgent for the user

### Service Configuration

The formula integrates with Homebrew Services using the `service` block:

```ruby
service do
  run [opt_bin/"rust-sci-hub-mcp", "--daemon", ...]
  working_dir "#{Dir.home}/Library/Application Support/rust-sci-hub-mcp"
  log_path "#{Dir.home}/Library/Logs/rust-sci-hub-mcp/stdout.log"
  error_log_path "#{Dir.home}/Library/Logs/rust-sci-hub-mcp/stderr.log"
  keep_alive crashed: true, successful_exit: false
  process_type :background
end
```

### Testing

The formula includes comprehensive tests:

- Binary existence and executability
- Version and help output validation
- Configuration file validity (TOML parsing)
- LaunchAgent plist validation

## Release Process

### Automated Release

1. **Tag the Release**: Create a Git tag (e.g., `v1.2.3`)
2. **Update Formula**: Run `./homebrew/update_formula.sh 1.2.3`
3. **Test Formula**: Build and test the updated formula
4. **Commit Changes**: Commit the updated formula and checksums
5. **Submit to Tap**: Push to the Homebrew tap repository

### Manual Release Steps

1. **Create Archive**: GitHub automatically creates release archives
2. **Calculate Checksum**: Download and calculate SHA256
3. **Update Formula**: Manually edit URL and SHA256 in formula
4. **Validate**: Run audit and style checks
5. **Test**: Install and test the formula

## Troubleshooting

### Common Issues

**Build Failures:**
- Check Rust toolchain compatibility
- Verify OpenSSL linkage
- Review build logs for specific errors

**Installation Issues:**
- Ensure Homebrew is up to date
- Check for conflicting dependencies
- Verify file permissions

**Service Problems:**
- Check LaunchAgent installation
- Verify configuration file validity
- Review service logs

### Debug Commands

```bash
# Check formula syntax
ruby -c homebrew/rust-sci-hub-mcp.rb

# Verbose installation
brew install --verbose homebrew/rust-sci-hub-mcp.rb

# Check dependencies
brew deps rust-sci-hub-mcp
brew missing rust-sci-hub-mcp

# Service debugging
brew services list
launchctl list | grep rust-sci-hub-mcp
```

## Contributing

### Formula Guidelines

Follow Homebrew's formula conventions:

- Use `opt_bin` for binary references
- Handle multiple architectures correctly
- Include appropriate test cases
- Follow Ruby style guidelines
- Use proper error handling

### Testing Changes

Before submitting changes:

1. Test on clean macOS installation
2. Verify both Intel and Apple Silicon compatibility
3. Run full audit and style checks
4. Test service integration
5. Validate uninstallation process

## Resources

- [Homebrew Formula Cookbook](https://docs.brew.sh/Formula-Cookbook)
- [Homebrew Services](https://github.com/Homebrew/homebrew-services)
- [Ruby Style Guide](https://rubystyle.guide/)
- [Homebrew Audit](https://docs.brew.sh/Brew-Audit)

## License

The Homebrew formula follows the same license as the main project (MIT).