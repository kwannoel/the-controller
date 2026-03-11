# Deployment Workspace Design

## Overview

A deployment workspace in The Controller that lets you deploy projects to Hetzner + Cloudflare via Coolify, with hotkey-driven deploys from sessions and a full infrastructure dashboard.

## Architecture

Three layers:

```
The Controller (local machine)
  → Coolify API (on Hetzner VPS)
    → Docker containers (your apps)
      → Cloudflare (CDN, DNS, DDoS)
```

### What Coolify handles (not built by us):
- Docker container lifecycle
- Reverse proxy (Traefik)
- SSL certificates
- Per-app secrets/env vars
- Database provisioning
- Logs, health checks, resource monitoring
- Rollback to previous deploys
- Server management, updates

### What The Controller builds (the UX layer):
- Deploy hotkey from sessions → triggers Coolify API
- First-deploy modal → project type detection, subdomain, production secrets
- Infrastructure workspace → dashboard pulling from Coolify API
- Convention-based config → auto-detect project type, generate Coolify deployment config
- Cloudflare integration → DNS record creation, proxy toggle
- Rollback hotkey → triggers Coolify rollback via API

## Project Type Detection

Convention-based detection, in priority order:

| Signal | Type | Deploy target |
|---|---|---|
| `index.html` or static site builder (Vite, Astro, Next export) | Static | Cloudflare Pages |
| `Dockerfile` present | Custom container | Hetzner via Coolify |
| `package.json` with `start` script | Node service | Hetzner via Coolify |
| `pyproject.toml` or `requirements.txt` with entry point | Python service | Hetzner via Coolify |

Static sites bypass Coolify entirely and deploy to Cloudflare Pages (free, edge-deployed).

If detection is ambiguous, the first-deploy modal asks the user to pick.

## Server Management & Networking

One Hetzner VPS runs all projects via Coolify.

**Recommended sizing:**

| Server | Specs | Price | Fits |
|---|---|---|---|
| CX22 | 2 vCPU, 4GB RAM | €3.79/mo | Coolify + 1-2 small apps |
| CX32 | 4 vCPU, 8GB RAM | €7.59/mo | Coolify + 3-5 apps |
| CX42 | 8 vCPU, 16GB RAM | €15.19/mo | Coolify + 10+ apps |

**Networking:**
- Cloudflare wildcard DNS: `*.yourdomain.com` → Hetzner server IP
- Cloudflare proxy enabled (orange cloud) — DDoS protection, caching
- Coolify's built-in Traefik handles reverse proxy and routing per subdomain
- TLS: Cloudflare edge → Coolify origin certificates

**Database:**
- SQLite with Litestream sidecar replicating to Cloudflare R2 (free tier: 10GB)
- Coolify also supports Postgres, MySQL, Redis if needed in the future

**Security (handled by Coolify + Cloudflare):**
- Coolify manages container isolation, updates, SSL
- Cloudflare proxy hides origin IP, provides DDoS protection
- Firewall: only 80, 443, SSH open
- SSH key auth only, no root login

## Deploy Flow

### First deploy (from a session):

```
1. Hit deploy hotkey (Leader → d)
2. Controller detects project type
3. First-deploy modal opens:
   - Auto-filled subdomain (project name → myapp.yourdomain.com)
   - Production secrets input (secure env modal UX)
   - Detected deploy type shown, option to override
   - "Deploy" button
4. Controller calls Coolify API:
   - Create new application resource
   - Set environment variables
   - Configure build settings
   - Trigger deploy
5. Controller calls Cloudflare API:
   - Create DNS record: myapp.yourdomain.com → server IP
   - Enable proxy
6. Deploy progress shown in notification
7. Health check passes → success with live URL
8. Health check fails → auto-rollback → error with logs
```

### Subsequent deploys (from a session):

```
1. Hit deploy hotkey
2. No modal — config already saved
3. Controller pushes latest code to Coolify via API
4. Coolify builds and deploys
5. Health check → success or auto-rollback
6. Notification
```

### Static sites (different path):

```
1. Hit deploy hotkey
2. Controller detects static site
3. Builds locally (npm run build)
4. Pushes dist/ to Cloudflare Pages API
5. No Coolify involved
```

### Rollback (from infrastructure workspace):

```
1. Space → Infrastructure workspace
2. Navigate to service with j/k
3. Hit r to rollback
4. Coolify rolls back to previous deployment
5. Health check confirms
```

## Infrastructure Workspace

New workspace (alongside Development, Agents, Notes, Architecture).

```
┌──────────────────────────────────────────────────┐
│  Sidebar              │  Infrastructure          │
│                       │                          │
│  Projects             │  ┌─ myapp ─────────────┐ │
│  ├─ myapp             │  │ ● Running  2d uptime │ │
│  ├─ agent-x           │  │ CPU: 3%  RAM: 128MB  │ │
│  └─ landing           │  │ Last deploy: 2m ago  │ │
│                       │  └──────────────────────┘ │
│                       │  ┌─ agent-x ────────────┐ │
│                       │  │ ● Running  5d uptime │ │
│                       │  │ CPU: 1%  RAM: 64MB   │ │
│                       │  │ Last deploy: 3d ago  │ │
│                       │  └──────────────────────┘ │
│                       │  ┌─ landing ────────────┐ │
│                       │  │ ☁ Cloudflare Pages   │ │
│                       │  │ Last deploy: 1w ago  │ │
│                       │  └──────────────────────┘ │
│                       │                          │
│                       │  [Logs panel below]      │
│                       │  > streaming stdout...   │
└──────────────────────────────────────────────────┘
```

**Hotkeys:**
- `j/k` — navigate between services
- `Enter` — expand service detail (full logs, deploy history)
- `r` — rollback selected service
- `d` — redeploy selected service
- `l` — focus log panel
- `s` — open production secrets
- `o` — open live URL in browser

## Onboarding Flow

One-time setup on first deploy attempt:

```
1. Leader → d (first ever)
2. Setup wizard modal:
   - Step 1: Hetzner API key (secure input)
   - Step 2: Cloudflare API key + root domain
   - Step 3: Controller provisions everything:
     → Creates Hetzner VPS via API
     → Installs Coolify via install script
     → Configures Cloudflare wildcard DNS
     → Stores credentials encrypted
   - Step 4: "Ready" (~3 minutes)
3. All subsequent deploys skip this
```

Credentials stored via The Controller's existing secure env system, encrypted at rest.

Server rebuild option available in infrastructure workspace — reprovisions everything, restores from Coolify backups and Litestream.

## Cost

- Hetzner VPS: €3.79–15.19/mo (depending on scale)
- Cloudflare: free (proxy, DNS, Pages, R2 up to 10GB)
- Coolify: free (self-hosted, open source)
- Domain: ~$10/yr
- **Total: under $10/mo for multiple SaaS products**
