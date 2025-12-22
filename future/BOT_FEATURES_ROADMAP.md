# Bot Enhancement Features Roadmap

> **Target Architecture**: Rust Core  
> **Last Updated**: December 22, 2025

Comprehensive feature list for WhatsApp and Telegram bot enhancements.

---

## Level 1: Basic Features

### Core Commands

- [ ] `/start` - Welcome message with bot introduction
- [ ] `/help` - List all available commands
- [ ] `/status` - System status and stats
- [ ] `/pending` - List pending matches
- [ ] `/confirm <id>` - Confirm a match
- [ ] `/reject <id>` - Reject a match

### Notifications

- [ ] New match alerts (score ≥ threshold)
- [ ] Urgent request notifications 🔥
- [ ] Daily summary digest

---

## Level 2: Intermediate Features

### Search & Query

- [ ] `/search <medication>` - Find offers/requests
- [ ] `/offers [medication]` - List active offers
- [ ] `/requests [medication]` - List active requests
- [ ] `/match <id>` - Get detailed match info

### Quick Actions

- [ ] `/c <id>` - Short confirm alias
- [ ] `/r <id>` - Short reject alias
- [ ] Inline keyboard confirm/reject (Telegram)
- [ ] Bulk confirm high-confidence matches

### User Preferences

- [ ] `/notify on|off` - Toggle notifications
- [ ] `/threshold <0.5-1.0>` - Set notification threshold
- [ ] `/language ar|en` - Switch language

---

## Level 3: Advanced Features

### Analytics & Reports

- [ ] `/report` - Generate daily report
- [ ] `/trending` - Top demanded medications
- [ ] `/performance` - Match confirmation rate

### Alert Configuration

- [ ] `/alert add <medication>` - Watch medication
- [ ] `/alert remove <medication>` - Stop watching
- [ ] `/alert list` - Show watched medications

### AI-Powered Queries

- [ ] Natural language search
- [ ] Context-aware suggestions
- [ ] Conversation-based workflows

---

## Level 4: Admin Features

### System Control

- [ ] `/pause` - Pause message processing
- [ ] `/resume` - Resume processing
- [ ] `/config` - View configuration
- [ ] `/config set <key> <value>` - Change config

### Group Management

- [ ] `/groups` - List known groups
- [ ] `/monitor <group>` - Start monitoring
- [ ] `/unmonitor <group>` - Stop monitoring

### User Management

- [ ] `/users` - List authorized users
- [ ] `/adduser <phone>` - Add user
- [ ] `/removeuser <phone>` - Remove user
- [ ] `/promote <phone>` - Promote to admin

---

## Implementation Priority

| Phase | Scope                            | Timeline |
| ----- | -------------------------------- | -------- |
| 1     | Core commands (Level 1)          | Week 1-2 |
| 2     | Search & quick actions (Level 2) | Week 3-4 |
| 3     | Admin features (Level 4)         | Week 5-6 |
| 4     | Advanced AI features             | Future   |

---

## Code Structure (Rust)

```
core/src/bot/
├── mod.rs              # Module exports
├── commands/
│   ├── mod.rs          # Command router
│   ├── basic.rs        # Level 1 commands
│   ├── search.rs       # Search commands
│   ├── admin.rs        # Admin commands
│   └── analytics.rs    # Report commands
├── middleware/
│   ├── auth.rs         # Authentication
│   └── ratelimit.rs    # Rate limiting
└── keyboards.rs        # Telegram inline keyboards
```

---

## Message Templates

### Match Notification

```
🔔 *New Match Found*
━━━━━━━━━━━━━━
💊 *Augmentin 1g*
📦 50 units @ 150 EGP
📋 Request: 20 units
📊 Score: *92%*
━━━━━━━━━━━━━━
ID: `abc12345`
Reply: /confirm abc12345
```

### Daily Summary

```
📊 *Daily Summary*
━━━━━━━━━━━━━━
✅ Confirmed: 12
⏳ Pending: 5
📦 New Offers: 23
📋 New Requests: 18
━━━━━━━━━━━━━━
Top: Augmentin (8 matches)
```

---

_Last Updated: December 22, 2025_
