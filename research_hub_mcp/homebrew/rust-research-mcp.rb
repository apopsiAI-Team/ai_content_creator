class RustResearchMcp < Formula
  desc "Rust-based MCP server for academic research paper access"
  homepage "https://github.com/Ladvien/research_mcp"
  url "https://github.com/Ladvien/research_mcp/archive/v0.2.0.tar.gz"
  sha256 "13d8ef4ab6a9ef456f5dc59b41a25a68ffad90a767e9240e57befd36dd0c36db"
  license "MIT"


  # Build dependencies
  depends_on "rust" => :build
  depends_on "pkg-config" => :build

  # Runtime dependencies
  depends_on "openssl@3"
  depends_on "curl"

  def install
    # Set environment variables for OpenSSL
    ENV["OPENSSL_DIR"] = Formula["openssl@3"].opt_prefix
    ENV["OPENSSL_LIB_DIR"] = Formula["openssl@3"].opt_lib
    ENV["OPENSSL_INCLUDE_DIR"] = Formula["openssl@3"].opt_include

    # Build the project
    system "cargo", "install", *std_cargo_args

    # Install LaunchAgent plist
    launchd_dir = prefix/"Library/LaunchAgents"
    launchd_dir.mkpath
    
    # Process plist template and install
    plist_source = buildpath/"launchd/com.rust-research-mcp.plist"
    plist_dest = launchd_dir/"com.rust-research-mcp.plist"
    
    if plist_source.exist?
      # Replace HOME_DIR placeholder with the actual installation prefix
      plist_content = plist_source.read
      plist_content.gsub!("HOME_DIR", var)
      plist_dest.write(plist_content)
    else
      odie "LaunchAgent plist template not found: #{plist_source}"
    end

    # Create necessary directories
    (var/"lib/rust-research-mcp").mkpath
    (var/"log/rust-research-mcp").mkpath
    (etc/"rust-research-mcp").mkpath

    # Install default configuration
    etc_config = etc/"rust-research-mcp/config.toml"
    unless etc_config.exist?
      etc_config.write <<~EOS
        # Rust Research MCP Server Configuration
        
        [server]
        host = "127.0.0.1"
        port = 8080
        health_check_interval_secs = 60
        graceful_shutdown_timeout_secs = 30
        
        [research_source]
        # Research source endpoints (will be auto-discovered if empty)
        endpoints = []
        timeout_secs = 30
        retry_attempts = 3
        rate_limit_requests_per_minute = 30
        
        [downloads]
        directory = "#{Dir.home}/Downloads/papers"
        concurrent_downloads = 3
        chunk_size_bytes = 8192
        
        [metadata]
        cache_enabled = true
        cache_ttl_hours = 168  # 1 week
        extraction_timeout_secs = 10
        
        [logging]
        level = "info"
        file = "#{var}/log/rust-research-mcp/service.log"
        max_size_mb = 10
        max_backups = 5
      EOS
    end

    # Install man page if it exists
    if (buildpath/"docs/rust-research-mcp.1").exist?
      man1.install "docs/rust-research-mcp.1"
    end

    # Install documentation
    doc.install "README.md"
    doc.install "docs/" if (buildpath/"docs").exist?
  end

  def post_install
    # Create user-specific directories
    user_config_dir = "#{Dir.home}/.config/rust-research-mcp"
    user_support_dir = "#{Dir.home}/Library/Application Support/rust-research-mcp"
    user_log_dir = "#{Dir.home}/Library/Logs/rust-research-mcp"

    system "mkdir", "-p", user_config_dir, user_support_dir, user_log_dir

    # Copy default config to user directory if it doesn't exist
    user_config_file = "#{user_config_dir}/config.toml"
    unless File.exist?(user_config_file)
      cp etc/"rust-research-mcp/config.toml", user_config_file
      system "chmod", "600", user_config_file
    end

    # Install LaunchAgent for user
    user_launchagents_dir = "#{Dir.home}/Library/LaunchAgents"
    system "mkdir", "-p", user_launchagents_dir

    user_plist = "#{user_launchagents_dir}/com.rust-research-mcp.plist"
    plist_source = prefix/"Library/LaunchAgents/com.rust-research-mcp.plist"
    
    if plist_source.exist?
      # Update plist with user's home directory
      plist_content = plist_source.read
      plist_content.gsub!(var.to_s, Dir.home)
      File.write(user_plist, plist_content)
      system "chmod", "644", user_plist
      
      ohai "LaunchAgent installed at: #{user_plist}"
      ohai "Note: Use 'brew services' instead of direct launchctl commands"
    else
      opoo "LaunchAgent plist not found, service management may be limited"
    end

    ohai "Installation complete!"
    ohai "Configuration file created at: #{user_config_file}"
    ohai "To start the service: brew services start rust-research-mcp"
    ohai "To check service status: brew services list | grep rust-research-mcp"
    ohai "Health check URL: http://localhost:8080/health"
  end

  def caveats
    <<~EOS
      The rust-research-mcp service has been installed but not started.
      
      To start the service:
        brew services start rust-research-mcp
      
      To start the service at login:
        brew services start rust-research-mcp --at-login
      
      Configuration file:
        #{Dir.home}/.config/rust-research-mcp/config.toml
      
      Logs:
        #{Dir.home}/Library/Logs/rust-research-mcp/
      
      Health check:
        curl http://localhost:8080/health
      
      For more information, see:
        #{doc}/README.md
    EOS
  end

  service do
    run [opt_bin/"rust-research-mcp", "--daemon", 
         "--config", "#{Dir.home}/.config/rust-research-mcp/config.toml",
         "--pid-file", "#{Dir.home}/Library/Application Support/rust-research-mcp/rust-research-mcp.pid"]
    working_dir "#{Dir.home}/Library/Application Support/rust-research-mcp"
    log_path "#{Dir.home}/Library/Logs/rust-research-mcp/stdout.log"
    error_log_path "#{Dir.home}/Library/Logs/rust-research-mcp/stderr.log"
    environment_variables RUST_LOG: "info"
    keep_alive crashed: true, successful_exit: false
    process_type :background
  end

  test do
    # Test that the binary was installed
    assert_predicate bin/"rust-research-mcp", :exist?
    assert_predicate bin/"rust-research-mcp", :executable?

    # Test version output
    version_output = shell_output("#{bin}/rust-research-mcp --version")
    assert_match "rust-research-mcp", version_output

    # Test help output
    help_output = shell_output("#{bin}/rust-research-mcp --help")
    assert_match "Rust-based MCP server", help_output

    # Test config file generation
    test_config_dir = testpath/".config/rust-research-mcp"
    test_config_dir.mkpath
    test_config_file = test_config_dir/"config.toml"
    
    # Copy the default config
    cp etc/"rust-research-mcp/config.toml", test_config_file
    assert_predicate test_config_file, :exist?

    # Test that config is valid TOML (basic syntax check)
    config_content = test_config_file.read
    assert_match(/\[server\]/, config_content)
    assert_match(/\[research_source\]/, config_content)
    assert_match(/host\s*=/, config_content)

    # Test LaunchAgent plist exists and is valid
    plist_file = prefix/"Library/LaunchAgents/com.rust-research-mcp.plist"
    assert_predicate plist_file, :exist?
    
    # Basic plist validation
    plist_content = plist_file.read
    assert_match "com.rust-research-mcp", plist_content
    assert_match "rust-research-mcp", plist_content
  end
end