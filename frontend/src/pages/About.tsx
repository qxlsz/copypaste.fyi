export const AboutPage = () => {
  return (
    <div className="min-h-screen bg-gray-50 dark:bg-slate-900">
      <div className="max-w-4xl mx-auto px-4 py-12 sm:px-6 lg:px-8">
        <div className="prose prose-lg dark:prose-invert max-w-none">
          <h1 className="text-4xl font-bold text-gray-900 dark:text-white mb-8">
            About copypaste.fyi
          </h1>

          <section className="mb-12">
            <h2 className="text-2xl font-semibold text-gray-800 dark:text-gray-200 mb-4">
              Motivation
            </h2>
            <p className="text-gray-600 dark:text-gray-400 leading-relaxed">
              Traditional paste-sharing services suffer from fundamental security and privacy limitations. 
              Most platforms rely on symmetric encryption with keys transmitted alongside URLs, creating 
              vulnerabilities in logging systems, browser history, and intermediate proxies. Furthermore, 
              centralized authentication systems introduce single points of failure and require users to 
              trust service providers with their identity.
            </p>
            <p className="text-gray-600 dark:text-gray-400 leading-relaxed mt-4">
              copypaste.fyi addresses these shortcomings through a cryptographically-sound, zero-knowledge 
              architecture that leverages Ed25519 elliptic curve cryptography for authentication and 
              ChaCha20-Poly1305 for content encryption. The system operates without traditional user 
              accounts, passwords, or recovery mechanisms—your cryptographic keys are your identity.
            </p>
          </section>

          <section className="mb-12">
            <h2 className="text-2xl font-semibold text-gray-800 dark:text-gray-200 mb-4">
              Technical Architecture
            </h2>
            <div className="space-y-6">
              <div>
                <h3 className="text-xl font-medium text-gray-700 dark:text-gray-300 mb-2">
                  Cryptographic Foundation
                </h3>
                <ul className="list-disc pl-6 space-y-2 text-gray-600 dark:text-gray-400">
                  <li>
                    <strong>Ed25519 Digital Signatures:</strong> Each user generates a 256-bit private key 
                    from which a corresponding public key is derived. Authentication occurs through 
                    challenge-response protocols where the server issues a random nonce that the client 
                    signs with their private key.
                  </li>
                  <li>
                    <strong>ChaCha20-Poly1305 AEAD:</strong> Content encryption employs authenticated 
                    encryption with additional data, providing both confidentiality and integrity. The 
                    256-bit keys are derived using cryptographically secure random number generation.
                  </li>
                  <li>
                    <strong>XChaCha20-Poly1305:</strong> Extended nonce variant supporting 192-bit nonces, 
                    eliminating nonce reuse concerns in high-volume scenarios while maintaining the same 
                    security guarantees.
                  </li>
                </ul>
              </div>

              <div>
                <h3 className="text-xl font-medium text-gray-700 dark:text-gray-300 mb-2">
                  Zero-Knowledge Design
                </h3>
                <p className="text-gray-600 dark:text-gray-400">
                  The server never receives plaintext content or encryption keys. All cryptographic 
                  operations occur client-side using the Web Crypto API and @noble/ed25519 library. 
                  The server stores only:
                </p>
                <ul className="list-disc pl-6 mt-2 space-y-1 text-gray-600 dark:text-gray-400">
                  <li>Encrypted content ciphertext</li>
                  <li>Public key hashes (SHA-256) for ownership verification</li>
                  <li>Metadata (creation time, expiration, format hints)</li>
                  <li>Access control parameters (burn-after-reading, time locks)</li>
                </ul>
              </div>

              <div>
                <h3 className="text-xl font-medium text-gray-700 dark:text-gray-300 mb-2">
                  Implementation Stack
                </h3>
                <p className="text-gray-600 dark:text-gray-400">
                  Backend: Rust with Rocket framework, leveraging ring for cryptographic primitives and 
                  RocksDB for persistent storage. The type system enforces security invariants at compile 
                  time, preventing entire classes of vulnerabilities.
                </p>
                <p className="text-gray-600 dark:text-gray-400 mt-2">
                  Frontend: React with TypeScript, ensuring type safety across the application boundary. 
                  Monaco Editor provides syntax highlighting for 100+ languages with lazy-loaded language 
                  definitions to optimize bundle size.
                </p>
              </div>
            </div>
          </section>

          <section className="mb-12">
            <h2 className="text-2xl font-semibold text-gray-800 dark:text-gray-200 mb-4">
              Security Advantages
            </h2>
            <div className="grid gap-4">
              <div className="border-l-4 border-green-500 pl-4">
                <h3 className="font-medium text-gray-700 dark:text-gray-300">
                  No Password Vulnerabilities
                </h3>
                <p className="text-gray-600 dark:text-gray-400 mt-1">
                  Eliminates password-based attacks, phishing, and credential stuffing. Private keys 
                  never leave the client device and cannot be recovered if lost—a feature, not a bug.
                </p>
              </div>

              <div className="border-l-4 border-blue-500 pl-4">
                <h3 className="font-medium text-gray-700 dark:text-gray-300">
                  Perfect Forward Secrecy
                </h3>
                <p className="text-gray-600 dark:text-gray-400 mt-1">
                  Each paste uses unique encryption keys. Compromise of one key reveals nothing about 
                  other pastes, even those created by the same user.
                </p>
              </div>

              <div className="border-l-4 border-purple-500 pl-4">
                <h3 className="font-medium text-gray-700 dark:text-gray-300">
                  Cryptographic Non-Repudiation
                </h3>
                <p className="text-gray-600 dark:text-gray-400 mt-1">
                  Ed25519 signatures provide mathematical proof of authorship. Users can prove ownership 
                  of content without revealing their private key.
                </p>
              </div>

              <div className="border-l-4 border-orange-500 pl-4">
                <h3 className="font-medium text-gray-700 dark:text-gray-300">
                  Tor Network Support
                </h3>
                <p className="text-gray-600 dark:text-gray-400 mt-1">
                  Native .onion address support with optional Tor-only access restrictions. Pastes can 
                  be configured to reject clearnet access entirely.
                </p>
              </div>
            </div>
          </section>

          <section className="mb-12">
            <h2 className="text-2xl font-semibold text-gray-800 dark:text-gray-200 mb-4">
              Use Cases
            </h2>
            <div className="space-y-4">
              <div className="bg-gray-100 dark:bg-slate-800 rounded-lg p-4">
                <h3 className="font-medium text-gray-700 dark:text-gray-300 mb-2">
                  Secure Configuration Sharing
                </h3>
                <p className="text-gray-600 dark:text-gray-400">
                  DevOps teams sharing sensitive configuration files, API keys, or infrastructure 
                  secrets with automatic expiration and burn-after-reading guarantees.
                </p>
              </div>

              <div className="bg-gray-100 dark:bg-slate-800 rounded-lg p-4">
                <h3 className="font-medium text-gray-700 dark:text-gray-300 mb-2">
                  Code Review & Debugging
                </h3>
                <p className="text-gray-600 dark:text-gray-400">
                  Sharing code snippets with syntax highlighting, line numbers, and format preservation. 
                  Time-locked pastes ensure temporary access for review purposes.
                </p>
              </div>

              <div className="bg-gray-100 dark:bg-slate-800 rounded-lg p-4">
                <h3 className="font-medium text-gray-700 dark:text-gray-300 mb-2">
                  Anonymous Whistleblowing
                </h3>
                <p className="text-gray-600 dark:text-gray-400">
                  Tor-only pastes with cryptographic proof of authenticity, enabling secure disclosure 
                  of sensitive information without revealing identity.
                </p>
              </div>

              <div className="bg-gray-100 dark:bg-slate-800 rounded-lg p-4">
                <h3 className="font-medium text-gray-700 dark:text-gray-300 mb-2">
                  Incident Response
                </h3>
                <p className="text-gray-600 dark:text-gray-400">
                  Security teams sharing indicators of compromise, logs, and forensic data with 
                  guaranteed destruction after analysis through burn-after-reading mechanisms.
                </p>
              </div>
            </div>
          </section>

          <section className="mb-12">
            <h2 className="text-2xl font-semibold text-gray-800 dark:text-gray-200 mb-4">
              Advanced Features
            </h2>
            <div className="space-y-6">
              <div>
                <h3 className="text-xl font-medium text-gray-700 dark:text-gray-300 mb-2">
                  Time-Lock Encryption
                </h3>
                <p className="text-gray-600 dark:text-gray-400">
                  Pastes can be configured with temporal access windows using not-before and not-after 
                  timestamps. The server enforces these constraints cryptographically, preventing premature 
                  or delayed access even if the database is compromised.
                </p>
              </div>

              <div>
                <h3 className="text-xl font-medium text-gray-700 dark:text-gray-300 mb-2">
                  Bundle Architecture
                </h3>
                <p className="text-gray-600 dark:text-gray-400">
                  Multiple related pastes can be grouped into bundles with hierarchical access control. 
                  Each bundle member maintains independent encryption while sharing a common namespace 
                  for organized content management.
                </p>
              </div>

              <div>
                <h3 className="text-xl font-medium text-gray-700 dark:text-gray-300 mb-2">
                  Attestation Requirements
                </h3>
                <p className="text-gray-600 dark:text-gray-400">
                  Support for TOTP-based attestation adds an additional authentication layer. Pastes 
                  can require time-based one-time passwords derived from shared secrets, enabling 
                  multi-factor protection without traditional authentication systems.
                </p>
              </div>

              <div>
                <h3 className="text-xl font-medium text-gray-700 dark:text-gray-300 mb-2">
                  Webhook Integration
                </h3>
                <p className="text-gray-600 dark:text-gray-400">
                  Configurable webhooks trigger on paste events (creation, access, expiration) enabling 
                  integration with external monitoring, alerting, and audit systems. Webhook payloads 
                  include cryptographic signatures for authenticity verification.
                </p>
              </div>
            </div>
          </section>

          <section className="mb-12">
            <h2 className="text-2xl font-semibold text-gray-800 dark:text-gray-200 mb-4">
              Privacy Guarantees
            </h2>
            <ul className="space-y-3 text-gray-600 dark:text-gray-400">
              <li className="flex items-start">
                <span className="text-green-500 mr-2">✓</span>
                <span>
                  <strong>No Analytics:</strong> Zero third-party tracking, analytics, or telemetry. 
                  The only statistics collected are aggregate counts for operational monitoring.
                </span>
              </li>
              <li className="flex items-start">
                <span className="text-green-500 mr-2">✓</span>
                <span>
                  <strong>No Logs:</strong> Access logs are disabled by default. When enabled for 
                  debugging, they exclude sensitive information like IP addresses or user agents.
                </span>
              </li>
              <li className="flex items-start">
                <span className="text-green-500 mr-2">✓</span>
                <span>
                  <strong>No Metadata Leakage:</strong> File names, MIME types, and other metadata 
                  are encrypted alongside content, preventing information disclosure through side channels.
                </span>
              </li>
              <li className="flex items-start">
                <span className="text-green-500 mr-2">✓</span>
                <span>
                  <strong>No Account Recovery:</strong> Lost keys cannot be recovered by design. This 
                  eliminates social engineering attacks and insider threats common to traditional services.
                </span>
              </li>
            </ul>
          </section>

          <section className="mb-12">
            <h2 className="text-2xl font-semibold text-gray-800 dark:text-gray-200 mb-4">
              Open Source Commitment
            </h2>
            <p className="text-gray-600 dark:text-gray-400 leading-relaxed">
              The entire codebase is open source and auditable. Security through obscurity is rejected 
              in favor of cryptographic proofs and mathematical guarantees. Contributors are encouraged 
              to review the implementation, suggest improvements, and report vulnerabilities through 
              responsible disclosure channels.
            </p>
            <p className="text-gray-600 dark:text-gray-400 leading-relaxed mt-4">
              The project adheres to the principle that security tools must be transparent to be 
              trustworthy. Every cryptographic decision, implementation detail, and architectural choice 
              is documented and open to scrutiny. This transparency enables security researchers and 
              users to verify claims independently rather than trusting assertions.
            </p>
          </section>

          <section className="mb-12">
            <h2 className="text-2xl font-semibold text-gray-800 dark:text-gray-200 mb-4">
              Future Roadmap
            </h2>
            <div className="space-y-3 text-gray-600 dark:text-gray-400">
              <div>
                <strong>Post-Quantum Cryptography:</strong> Migration path to quantum-resistant algorithms 
                including CRYSTALS-Dilithium for signatures and Kyber for key encapsulation.
              </div>
              <div>
                <strong>Distributed Architecture:</strong> Federation support allowing multiple instances 
                to share content while maintaining cryptographic sovereignty.
              </div>
              <div>
                <strong>Hardware Security Module Integration:</strong> Support for HSM-backed key storage 
                and cryptographic operations for enterprise deployments.
              </div>
              <div>
                <strong>Formal Verification:</strong> Mathematical proofs of security properties using 
                tools like ProVerif and Tamarin for protocol analysis.
              </div>
            </div>
          </section>
        </div>
      </div>
    </div>
  )
}
