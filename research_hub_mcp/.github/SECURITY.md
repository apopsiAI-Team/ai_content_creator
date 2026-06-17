# Security Policy

## Supported Versions

We provide security updates for the following versions:

| Version | Supported          |
| ------- | ------------------ |
| 0.4.x   | ✅ Yes            |
| 0.3.x   | ⚠️ Limited support |
| < 0.3   | ❌ No             |

## Reporting a Vulnerability

We take security seriously. If you discover a security vulnerability in rust-research-mcp, please follow responsible disclosure practices:

### 🔒 Private Reporting (Preferred)

1. **Email**: Send details to [security contact - to be added]
2. **GitHub**: Use GitHub's private vulnerability reporting feature
3. **Include**:
   - Description of the vulnerability
   - Steps to reproduce
   - Potential impact
   - Suggested fix (if any)

### ⏱️ Response Timeline

- **Initial Response**: Within 48 hours
- **Triage**: Within 1 week
- **Fix Development**: Depends on severity
- **Public Disclosure**: After fix is released

### 🚨 Severity Classification

**Critical** - Remote code execution, data corruption
- Response: Immediate (24-48 hours)
- Fix: Emergency patch

**High** - Privilege escalation, authentication bypass
- Response: Within 1 week
- Fix: Next patch release

**Medium** - Information disclosure, DoS
- Response: Within 2 weeks
- Fix: Next minor release

**Low** - Minor information leaks
- Response: Within 1 month
- Fix: Next major release

## Security Considerations

### Academic Content Access

This tool accesses academic content sources. Please note:

- **Legal Compliance**: Ensure your use complies with local laws
- **Terms of Service**: Respect provider terms and rate limits
- **Institutional Policies**: Follow your institution's guidelines
- **Personal Use**: Tool is designed for personal academic research only

### Data Handling

- **No Persistence**: No user data is stored persistently
- **Temporary Files**: Downloaded files are stored locally only
- **Network Security**: All connections use HTTPS where possible
- **Input Validation**: All inputs are validated and sanitized

### Dependencies

We regularly audit our dependencies for security vulnerabilities:

- **Automated Scanning**: GitHub Dependabot alerts
- **Manual Review**: Regular `cargo audit` checks
- **Updates**: Prompt updates for security fixes

### Configuration Security

- **File Permissions**: Config files should be readable only by owner (0600)
- **API Keys**: Store securely, never commit to version control
- **Logging**: Sensitive data is not logged
- **Default Settings**: Secure by default configuration

## Best Practices for Users

### Installation

- **Verify Downloads**: Check checksums for binary releases
- **Build from Source**: Consider building from audited source
- **Regular Updates**: Keep software updated

### Configuration

- **Minimal Permissions**: Run with least privilege necessary
- **Secure Storage**: Protect configuration files
- **API Key Management**: Use environment variables for sensitive data

### Network Security

- **Firewall**: Configure appropriate network restrictions
- **VPN**: Consider using VPN for additional privacy
- **Monitoring**: Monitor network traffic for anomalies

## Vulnerability History

We maintain a public record of security vulnerabilities and fixes:

### 2024
- No security vulnerabilities reported

## Security Contacts

- **Primary**: [To be added]
- **Backup**: Open GitHub issue for non-sensitive reports

## Acknowledgments

We appreciate security researchers who responsibly disclose vulnerabilities. Depending on the severity and impact, we may:

- Acknowledge your contribution in release notes
- Add you to our security researchers hall of fame
- Coordinate with you on public disclosure timing

## Legal and Ethical Guidelines

### Academic Use Only

This tool is designed for legitimate academic research. Security reports should:

- **Focus on Tool Security**: Report vulnerabilities in the tool itself
- **Respect Provider Terms**: Don't report issues that violate provider ToS
- **Academic Context**: Consider the academic research context

### Responsible Disclosure

- **No Public Disclosure**: Don't publish vulnerabilities before fixes
- **No Exploitation**: Don't use vulnerabilities for malicious purposes
- **Coordination**: Work with us on responsible timeline

## Additional Resources

- [OWASP Security Guidelines](https://owasp.org/)
- [Rust Security Guidelines](https://anssi-fr.github.io/rust-guide/)
- [GitHub Security Documentation](https://docs.github.com/en/code-security)

---

Thank you for helping keep rust-research-mcp secure! 🔒