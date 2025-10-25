# CodePaste.fyi - Developer Code Sharing Platform

A terminal extension (iTerm/Warp) that enables developers to instantly share code snippets with syntax highlighting, self-destructing options, and real-time log streaming capabilities.

## Project Vision
Create a seamless way for developers to share terminal output and code snippets with team members through simple, shareable links with real-time updates.

## Core Features

### Code Sharing & Syntax Highlighting
- [ ] **Multi-language Support**
  - [ ] First-class support for:
    - [ ] C++ (including C++11/14/17/20 features)
    - [ ] Go (with module support)
  - [ ] Additional language support:
    - [ ] JavaScript/TypeScript (with JSX/TSX support)
    - [ ] Java (including modern Java features)
    - [ ] C# (.NET Core, .NET 5+)
    - [ ] Python
    - [ ] Ruby
    - [ ] Rust
  - [ ] Automatic language detection
  - [ ] Manual language override
  - [ ] Line number toggling
  - [ ] Copy button for quick code copying

### Self-Destruct & Privacy Features
- [ ] **Expiration Options**
  - [ ] One-time view (auto-delete after first view)
  - [ ] Time-based expiration (1h, 6h, 12h, 24h, 1 week, custom)
  - [ ] View count limit (1-1000 views)
  - [ ] Manual deletion option
- [ ] **Access Control**
  - [ ] Password protection
  - [ ] Email notification on view
  - [ ] IP-based restrictions
  - [ ] Referrer restrictions
- [ ] **Burn After Reading**
  - [ ] Client-side encryption option
  - [ ] Zero-knowledge proof of deletion
  - [ ] Audit logs for sensitive operations

### Phase 1: MVP (Weeks 1-4)
- [ ] **Terminal Extension (iTerm2)**
  - [ ] Basic code highlighting for top 5 languages (C++, Go, JavaScript, Java, C#)
  - [ ] Shareable link generation with copy-to-clipboard
  - [ ] Default 24h expiration
  - [ ] Command palette integration
  - [ ] Basic error handling and user feedback

- [ ] **Web Viewer**
  - [ ] Syntax highlighting powered by Monaco Editor
  - [ ] Responsive design (desktop/mobile)
  - [ ] Dark/light theme support
  - [ ] Basic metadata display (creation time, language, size)
  - [ ] Raw text view option
  - [ ] Download as file option

- [ ] **Backend Services**
  - [ ] REST API for snippet creation/retrieval
  - [ ] Rate limiting and abuse prevention
  - [ ] Basic analytics (view count, user agent)
  - [ ] Database schema for snippets and metadata

### Phase 2: Enhanced Features (Weeks 5-8)
- [ ] **Extended Terminal Support**
  - [ ] Native Warp extension
    - [ ] Custom UI components in Warp
    - [ ] Command palette integration
    - [ ] Status bar indicators
  - [ ] VS Code extension
  - [ ] JetBrains plugin

- [ ] **Advanced Sharing Controls**
  - [ ] Custom expiration presets
  - [ ] Password protection with strength meter
  - [ ] Email notifications on view
  - [ ] Public/Unlisted/Private visibility
  - [ ] Team sharing options

- [ ] **Content Management**
  - [ ] Multiple files per paste
  - [ ] Directory structure preservation
  - [ ] Drag-and-drop upload
  - [ ] Paste from clipboard history
  - [ ] Bulk operations

### Phase 3: Real-time & Collaboration (Weeks 9-12)
- [ ] **Live Log Streaming**
  - [ ] WebSocket-based real-time updates
  - [ ] Terminal emulation in browser
  - [ ] Multiple concurrent viewers
  - [ ] Search and filter logs
  - [ ] Ansi color support
  - [ ] Terminal resizing support

- [ ] **Terminal Session Sharing**
  - [ ] Read-only sharing
  - [ ] Collaborative terminal (opt-in)
  - [ ] Session recording and playback
  - [ ] Permission management
  - [ ] Terminal state synchronization

### Phase 4: Advanced Features
- [ ] Authentication & Teams
  - User accounts
  - Team workspaces
  - Access controls
- [ ] API Access
  - REST API for integration
  - Webhooks for events
  - CLI tool for automation
- [ ] Analytics
  - View counts
  - Geographic data
  - Popular snippets

## Technical Architecture

### Core Components
- **Frontend**
  - React 18+ with TypeScript
  - Monaco Editor for code editing
  - TailwindCSS for styling
  - WebSocket for real-time features
  - Service Workers for offline support

- **Backend**
  - Node.js 18+ with Express
  - TypeScript for type safety
  - WebSocket server
  - Rate limiting middleware
  - Request validation

- **Data Layer**
  - PostgreSQL for metadata and persistent storage
  - Redis for:
    - Caching (code snippets, API responses)
    - Rate limiting
    - Real-time features via Redis Pub/Sub
    - Session storage
    - Job queues (BullMQ)
  - S3-compatible storage for content
  - Prisma ORM for database access
  - Redis OM for Redis object mapping

- **Infrastructure**
  - Docker containers
  - Kubernetes for orchestration
  - Prometheus + Grafana for monitoring
  - GitHub Actions for CI/CD
  - Terraform for infrastructure as code

### Security Considerations
- End-to-end encryption for sensitive content
- Regular security audits
- DDoS protection
- Rate limiting
- Input sanitization
- CSRF protection
- CSP headers
- Security.txt implementation
- **Frontend**: React + TypeScript
- **Backend**: Node.js + Express
- **Database**: PostgreSQL
- **Real-time**: WebSockets
- **Storage**: Object storage (S3 compatible)
- **Deployment**: Docker + Kubernetes

## Development Roadmap

### Phase 1: Core Functionality
- [ ] **Terminal Extension (iTerm2 & Warp)**
  - [ ] Basic code highlighting for multiple languages
  - [ ] Shareable link generation
  - [ ] Command palette integration
  - [ ] Error handling and user feedback
  - [ ] Basic configuration options

- [ ] **Web Viewer**
  - [ ] Syntax highlighting with Monaco Editor
  - [ ] Responsive design for all devices
  - [ ] Dark/light theme support
  - [ ] Raw text and download options
  - [ ] Basic metadata display

- [ ] **Backend Services**
  - [ ] REST API v1.0
  - [ ] Authentication system
  - [ ] Rate limiting and abuse prevention
  - [ ] Database schema and migrations
  - [ ] Basic analytics

### Phase 2: Enhanced Features
- [ ] **Advanced Sharing**
  - [ ] Custom expiration settings
  - [ ] Password protection
  - [ ] One-time view (burn after reading)
  - [ ] View count limits
  - [ ] Email notifications

- [ ] **Content Management**
  - [ ] Multiple files per paste
  - [ ] Directory structure preservation
  - [ ] Bulk operations
  - [ ] Search functionality
  - [ ] User dashboard

- [ ] **Developer Experience**
  - [ ] VS Code extension
  - [ ] JetBrains plugin
  - [ ] CLI tool
  - [ ] Browser extension
  - [ ] Comprehensive API documentation

### Phase 3: Real-time Collaboration
- [ ] **Live Features**
  - [ ] WebSocket-based updates
  - [ ] Real-time viewer count
  - [ ] Terminal session sharing
  - [ ] Collaborative editing
  - [ ] Live cursor presence

- [ ] **Team & Organization**
  - [ ] User accounts and profiles
  - [ ] Team workspaces
  - [ ] Access controls and permissions
  - [ ] Audit logs
  - [ ] Team analytics

- [ ] **Advanced Security**
  - [ ] End-to-end encryption
  - [ ] IP whitelisting
  - [ ] Session management
  - [ ] Two-factor authentication
  - [ ] Compliance features

### Phase 4: Ecosystem & Scale
- [ ] **Integration**
  - [ ] GitHub/GitLab integration
  - [ ] CI/CD pipeline support
  - [ ] Webhook system
  - [ ] API v2.0 with webhooks
  - [ ] Plugin system

- [ ] **Advanced Features**
  - [ ] AI-powered code explanations
  - [ ] Code review tools
  - [ ] Documentation generation
  - [ ] Performance benchmarking
  - [ ] Custom domain support

- [ ] **Deployment Options**
  - [ ] Self-hosted version
  - [ ] Enterprise features
  - [ ] High-availability setup
  - [ ] Multi-region support
  - [ ] Backup and recovery

### Phase 5: Community & Growth
- [ ] **Community Features**
  - [ ] Public snippet sharing
  - [ ] User profiles and following
  - [ ] Comments and discussions
  - [ ] Code templates
  - [ ] Learning resources

- [ ] **Monetization**
  - [ ] Pro features
  - [ ] Team plans
  - [ ] Enterprise support
  - [ ] API usage tiers
  - [ ] Custom development services

- [ ] **Ecosystem**
  - [ ] Mobile applications
  - [ ] Desktop applications
  - [ ] IDE plugins for all major IDEs
  - [ ] Browser developer tools
  - [ ] Community plugins marketplace

## Success Metrics
- Number of active users
- Snippets created per day
- Average snippet lifetime
- API response times
- Error rates
- User retention

## Future Considerations
- Self-hosted version
- Desktop applications
- Browser extension
- IDE plugins for all major IDEs
- API rate limit management
- Webhook integrations
- GitHub/GitLab integrations
- Team collaboration features
- Code review tools
- Documentation generation
- AI-powered code explanations

### Milestone 1: Core Functionality (4 weeks)
- Basic terminal extension working
- Web viewer with syntax highlighting
- Simple sharing mechanism

### Milestone 2: Enhanced Features (8 weeks)
- Warp terminal support
- Customizable link settings
- Basic analytics

### Milestone 3: Real-time (12 weeks)
- Live log streaming
- Terminal session sharing
- Team collaboration features

## Getting Started

### Prerequisites
- Node.js 18+
- pnpm 8+
- Docker & Docker Compose
- PostgreSQL 14+
- Redis 7.0+ (included in Docker setup)
- RedisInsight (optional, for Redis GUI)

### Local Development with Docker

#### Quick Start
```bash
# Clone the repository
git clone https://github.com/yourusername/copypaste.fyi.git
cd copypaste.fyi

# Copy and configure environment variables
cp .env.example .env
# Edit .env as needed

# Start all services (PostgreSQL, Redis, RedisInsight, and App)
docker-compose up -d

# Run database migrations
docker-compose exec app pnpm db:migrate

# Access the application
# Web UI: http://localhost:3000
# RedisInsight (GUI for Redis): http://localhost:8001
# PostgreSQL: localhost:5432
# Redis: localhost:6379
```

#### Redis Configuration
Redis is pre-configured in the Docker setup with:
- Persistent storage (mounted to `./data/redis`)
- Password protection (set via `REDIS_PASSWORD` in .env)
- Memory limits and eviction policies
- Health checks

#### Local Development Commands
```bash
# Start services in detached mode
docker-compose up -d

# View logs
docker-compose logs -f

# Stop services
docker-compose down

# Rebuild containers
docker-compose up -d --build

# Access Redis CLI
docker-compose exec redis redis-cli

# Run tests with Redis
docker-compose exec app pnpm test:redis
```

#### Environment Variables
```env
# Redis Configuration
REDIS_URL=redis://:${REDIS_PASSWORD}@redis:6379
REDIS_PASSWORD=your_secure_password
REDIS_TTL=86400  # Default TTL in seconds (24h)

# Cache Configuration
CACHE_ENABLED=true
CACHE_TTL=3600  # 1 hour
CACHE_PREFIX=cpfy:

# Rate Limiting
RATE_LIMIT_WINDOW=15  # minutes
RATE_LIMIT_MAX=100    # requests per window
```

### Testing
```bash
# Run all tests
docker-compose exec app pnpm test

# Run specific test suites
docker-compose exec app pnpm test:unit
docker-compose exec app pnpm test:integration
docker-compose exec app pnpm test:e2e

# Test Redis integration
docker-compose exec app pnpm test:redis

# Generate coverage report
docker-compose exec app pnpm test:coverage

# Run tests with watch mode
docker-compose exec app pnpm test:watch
```

### Production Deployment

#### Docker Compose (Recommended)
```bash
# Build and start production services
docker-compose -f docker-compose.yml -f docker-compose.prod.yml up -d --build

# View logs
docker-compose logs -f

# Run migrations
docker-compose exec app pnpm db:migrate:prod

# Create admin user
docker-compose exec app pnpm cli create-admin
```

#### Kubernetes (Optional)
```bash
# Deploy to Kubernetes
kubectl apply -f k8s/

# View pods
kubectl get pods

# View services
kubectl get svc
```

#### Redis Configuration for Production
1. Enable Redis persistence
2. Set up Redis Sentinel for high availability
3. Configure memory limits and eviction policies
4. Set up monitoring with RedisInsight or similar
5. Enable TLS for secure connections

#### Monitoring
- Redis metrics exposed at `/metrics` endpoint
- Health check at `/health`
- Prometheus and Grafana dashboards available
- Log aggregation with ELK stack

## Contributing

We welcome contributions from the community! Here's how you can help:

### Reporting Issues
- Check existing issues before creating a new one
- Include steps to reproduce the issue
- Provide error messages and environment details
- Include screenshots or screen recordings if applicable

### Code Contributions
1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

### Code Style
- Follow the existing code style
- Write meaningful commit messages
- Include tests for new features
- Update documentation as needed
- Keep pull requests focused on a single feature/bugfix

### Development Workflow
1. Create an issue describing the feature/bug
2. Assign the issue to yourself
3. Work on your feature branch
4. Add tests and update documentation
5. Submit a pull request
6. Address review comments
7. Get your PR merged!

## License
MIT
