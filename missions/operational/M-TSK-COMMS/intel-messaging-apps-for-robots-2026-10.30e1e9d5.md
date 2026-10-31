# Messaging apps for robots: how a Claude Code cloud session can message Jim and receive replies

Date of facts: 30 October 2026. Prepared for mission M-TSK-COMMS-01 (tsk). No software was installed and no message was sent to any service. Each fact carries a URL. Marks used: "read" means I fetched the page; "search summary" means the claim comes only from a web search result summary; "failed" means the page returned an error; "unverified" means I found no source.

## Summary

**Is there an MCP server for Discord?** Yes, several, all community-made except one plugin that Anthropic ships. Anthropic's own Discord plugin (`discord@claude-plugins-official`) is a channel plugin, not a general MCP server (read: https://code.claude.com/docs/en/channels). Community servers: SaseQ/discord-mcp (Java, MIT, 574 stars on the page I read; send, read, search, threads, direct messages; bot token) (read: https://github.com/SaseQ/discord-mcp). The official MCP registry lists others, for example GuildControl and two Smithery-hosted servers (read: https://registry.modelcontextprotocol.io/v0/servers?search=discord&limit=10). None of these listens for new messages in the way a channel does; they let the model call send and read tools when it is already running. The claude.ai connector registry has no Discord connector (given in the brief, not re-checked by me).

**Can Jim give a robot a Discord account?** Give it a bot account, not a user account. Discord forbids automating a normal user account ("self-bots") (search summary of a Discord help article; the page itself returned 403: https://support.discord.com/hc/articles/115002192352). A bot account is created in the Developer Portal and uses a bot token (read: https://code.claude.com/docs/en/channels). I could not read the Developer Policy text (403 on every route), so the exact wording is not quoted here.

**Simplest way for a cloud-session robot to message Jim on Discord.** The robot calls Discord's HTTP API with the bot token: open a DM channel with Jim's user ID (`POST /users/@me/channels`), then post a message. This needs outbound HTTPS to `discord.com` only, which the Trusted network level does not include (the default allowed domain list I read has no Discord entry), so the environment needs the Custom level with `discord.com` added (read: https://code.claude.com/docs/en/cloud-environments). Sending needs no websocket. Whether Discord lets a bot open a DM with a user it shares no server with is unverified; the Claude Discord plugin says Jim must share a server with the bot before he can DM it (read: https://github.com/anthropics/claude-plugins-official/tree/main/external_plugins/discord).

**What Jim's reply needs.** A running process must hold a Discord Gateway websocket (or Discord must call an HTTP endpoint) to receive the reply, and something must then start or wake a robot. A cloud session cannot hold the connection once its VM pauses after a few minutes idle (read: https://code.claude.com/docs/en/cloud-environments). The routes into a cloud session are a scheduled task API trigger (`/fire`, starts a new session, 100 fires an hour, beta), a schedule, a GitHub event (read: https://code.claude.com/docs/en/routines), and in this environment `watch_url` (from the tool description in this session; not in public docs I read). So a reply path needs a small always-on relay that holds the Gateway and calls `/fire` or `watch_url`. That is a design choice for the reader.

**Does Claude Code already ship a way?** Partly, and not for cloud sessions. Channels (research preview) connect a running Claude Code session to Telegram, Discord or iMessage, with a sender allowlist and optional permission relay. A channel is an MCP server that Claude Code spawns as a subprocess over stdio on the same machine, and the docs say events arrive only while the session is open (read: https://code.claude.com/docs/en/channels-reference). The docs describe it for a running terminal session; they do not say it works in cloud sessions, and the plugin needs the `--channels` launch flag and Bun. Remote Control plus mobile push notifications work for sessions on Jim's own machine and send a push to the Claude mobile app (read: https://code.claude.com/docs/en/remote-control). Claude Tag starts cloud sessions from Slack only (read: https://code.claude.com/docs/en/changelog).

**Which alternative suits Jim best.** On cost, terms and separation from work Slack, Telegram is the closest fit for two-way chat: a free bot API with long polling or webhooks, no business verification, and a bot may message a user who started the chat first (read: https://core.telegram.org/bots/faq). It is also in Anthropic's channels set. For one-way alerts with almost no setup, ntfy or Pushover fit; neither takes a reply as free text (ntfy action buttons send an HTTP request; not a typed reply). Discord is a fair second choice if Jim already uses it. WhatsApp and SMS cost money and need business set-up. Signal has no official bot API. Details are in section 4.

---

## 1. Discord

### 1.1 MCP servers

- Anthropic's Discord plugin for channels: tools `reply`, `react`, `edit_message`, `fetch_messages` (up to 100 per call, oldest first), `download_attachment`. It needs Bun, a bot token, the Message Content Intent, and the launch flag `claude --channels plugin:discord@claude-plugins-official`. Read: https://github.com/anthropics/claude-plugins-official/tree/main/external_plugins/discord (page read through a summariser; the licence was not stated on the page) and https://code.claude.com/docs/en/channels.
- SaseQ/discord-mcp: 574 stars, MIT, 115 commits, bot token via `DISCORD_TOKEN`. Tools include `send_message`, `read_messages`, `edit_message`, `delete_message`, `search_messages`, threads (`list_active_threads`, `list_threads`, `get_thread_info`), and direct messages (`send_private_message`, `read_private_messages`, edit and delete). The page does not show the last commit date and shows no real-time listener. Read: https://github.com/SaseQ/discord-mcp. A search summary gave 336 stars and an update date of 2026-04-25 from a directory site (https://enterprisedna.co/directories/mcp/saseq-discord-mcp); the two star counts conflict, and I trust the GitHub page more. My direct GitHub API call for the star count failed (the shell command was cut off), so the figure is as of the page fetch.
- Official MCP registry (read through its API): `ai.smithery/Danushkumar-V-mcp-discord` and `ai.smithery/sachicali-discordmcp-suite` (hosted remote servers behind a Smithery API key), and `app.lasers.guildcontrol/discord` (GuildControl 0.3.1, npm `guildctl`, uses `DISCORD_BOT_TOKEN`, "privacy-safe reads, audits, and reviewed administration"). URL: https://registry.modelcontextprotocol.io/v0/servers?search=discord&limit=10. The output was cut off, so more entries may exist.
- Other names from a search summary only: cappyeo/discord-mcp (npm `@discord-mcp/cli`) and hypark5540/discord-mcp (read-focused). Source: web search "Discord MCP server GitHub" (search summary; star counts may be stale). All needed a bot token according to that summary.
- The modelcontextprotocol/servers repository: I did not check whether it lists a Discord server. Unverified.
- claude.ai connectors: no Discord connector (brief; I did not re-run that search). MCP connectors enabled on a session run through Anthropic's servers and need no allowlist entry (read: https://code.claude.com/docs/en/cloud-environments), so a hosted connector would be the one path that avoids the network level.

### 1.2 Bot token versus user token, and Discord's rules

- Bots authenticate with a bot token; the Claude plugin's setup uses the Developer Portal, "Reset Token", and an OAuth2 `bot` scope invite. Read: https://code.claude.com/docs/en/channels.
- Self-bots: a Discord help article states that automating normal user accounts is forbidden and can lead to account termination; Discord's Community Guidelines list "Do not use self-bots or user-bots. Each account must be associated with a human, not a bot." Source: search summary, https://support.discord.com/hc/articles/115002192352 (the page returned 403 to me) and https://discord.com/terms/guidelines (search summary only).
- Developer Policy: I could not read it. https://discord.com/developers/docs/policies-and-agreements/developer-policy redirected (301) to https://docs.discord.com/developers/policies-and-agreements/developer-policy, which redirected (307) to https://support-dev.discord.com/hc/articles/8563934450327-Discord-Developer-Policy, which returned 403, as did https://support-dev.discord.com/hc/en-us/articles/8563934450327-Discord-Developer-Policy. A search summary says the policy has a rule against modifying a user's account without explicit permission and that Discord may take enforcement action against violating apps; indexed copies are dated 1 October 2022 and 8 July 2024. It says nothing I could confirm about AI use of message content. Treat the exact wording as a gap. Source: search summary of https://support-dev.discord.com/hc/articles/25280499088279 and https://discord.com/safety/important-policy-updates.
- Discord's policy-updates page says the company does not allow AI companies to train models on user messages (search summary only; concerns AI companies, not bot developers).

### 1.3 How a bot receives messages

- Gateway: a persistent WebSocket. The app gets a WSS URL from `/gateway/bot`, receives Hello with a heartbeat interval, sends Identify with token and intents, gets Ready, and heartbeats until it disconnects. After a drop it reconnects to `resume_gateway_url` and sends Resume. Read: https://docs.discord.com/developers/events/gateway.
- Gateway limits: 120 events per connection per 60 seconds, immediate disconnect when exceeded; Identify capped at 1000 per 24 hours across shards. Read: same URL.
- Interactions webhooks (inbound HTTP): I did not fetch the interactions page, so I cannot state from a source how they work. The rate-limit page says interaction endpoints do not count against the global limit (read: https://docs.discord.com/developers/topics/rate-limits). Interactions cover slash commands and buttons, not free text messages (my understanding; unverified in a source I read).
- Message Content intent: privileged. Without it the app gets empty `content`, `embeds` and `attachments`, except for messages the app sends, DMs with the app, messages that mention the app, and message context-menu targets. Apps in 100 or more guilds need verification and approval. Read: https://docs.discord.com/developers/events/gateway. For one person's private bot, the toggle in the Developer Portal is enough.

### 1.4 Can a bot DM Jim

- `POST /users/@me/channels` with `recipient_id` opens a DM channel, or returns the existing one. Discord says not to use it to DM everyone in a server, and that DMs "should generally be started by a user action"; opening many DMs quickly can get the bot rate limited or blocked from opening new DMs. Read: https://docs.discord.com/developers/resources/user.
- Whether a shared server is required is not stated there. The Claude plugin says the bot must share a server with Jim because Discord does not let a user DM a bot otherwise (read: https://github.com/anthropics/claude-plugins-official/tree/main/external_plugins/discord). Error 50007 means the user has DMs closed or blocked the bot (search summary: https://github.com/python-discord/bot/issues/1640). Practical set-up: a private server with Jim and the bot.

### 1.5 Rate limits

- Global: 50 requests per second per bot. Per-route limits are not listed and change; read the `X-RateLimit-*` headers and honour `Retry-After` on 429. Cloudflare bans an IP at 10,000 invalid requests (401, 403, 429) in 10 minutes. Read: https://docs.discord.com/developers/topics/rate-limits. These limits are far above what a robot that messages one person needs.

---

## 2. Claude Code's own support

All items below were read in the docs on 30 October 2026 unless marked.

- **Channels.** "Channels are in research preview." A channel is an MCP server that pushes events into a running session; two-way channels expose a reply tool. Telegram, Discord and iMessage plugins are included, plus a localhost demo called fakechat. Each needs Bun. Flags `--channels` and `--dangerously-load-development-channels` do not appear in `claude --help`. Needs claude.ai or Console authentication; not on Bedrock, Google Cloud's Agent Platform or Microsoft Foundry. On Team and Enterprise an Owner must enable them; Pro and Max users without an organisation skip those checks. Only plugins on an Anthropic allowlist register (the community marketplace is not on it). URL: https://code.claude.com/docs/en/channels. A search summary says the feature was announced on 20 March 2026 and needs Claude Code v2.1.80 or later (https://implicator.ai/anthropic-ships-its-openclaw-rival-connecting-claude-code-to-telegram-and-discord; search summary only).
- **Where a channel runs.** "A channel is an MCP server that runs on the same machine as Claude Code. Claude Code spawns it as a subprocess and communicates over stdio." The Telegram and Discord plugins poll or connect from that machine; there is "no URL to expose". "Events only arrive while the session is open." URLs: https://code.claude.com/docs/en/channels-reference and https://code.claude.com/docs/en/channels. The docs call the target "your running Claude Code session" and compare channels with cloud sessions as separate features. They contain no statement that channels work in cloud sessions. I treat that as not supported or not documented. The `-p` mode can run channels, but the development flag is ignored there.
- **Permission relay.** A two-way channel can forward tool-approval prompts to the chat app; anyone who can reply through the channel can approve or deny tool use, so only trusted senders belong on the allowlist. A `yes <id>` or `no <id>` reply carries a five-letter request ID. URL: https://code.claude.com/docs/en/channels-reference.
- **Remote Control.** Connects claude.ai/code or the Claude mobile app to a session running on Jim's own machine; the local `claude` process must keep running. Available on Pro, Max, Team, Enterprise; API keys not supported. It does not apply to cloud sessions. URL: https://code.claude.com/docs/en/remote-control. The changelog shows Remote Control fixes through 2.1.314 (30 October 2026) with no status label (https://code.claude.com/docs/en/changelog; the page was truncated at 100,000 of 1,141,410 characters, so older 2026 entries were not read).
- **Mobile push notifications.** "When Remote Control is active, Claude can send push notifications to your phone", typically when a long task finishes or it needs a decision; set with "Push when Claude decides" in `/config` or by asking. Introduced in the week of 13 to 17 April 2026 (v2.1.105 to v2.1.113). URLs: https://code.claude.com/docs/en/remote-control#mobile-push-notifications and https://code.claude.com/docs/en/whats-new/2026-w16. The docs I read do not name a `PushNotification` tool, and they tie the feature to an active Remote Control session. Whether a cloud session can send a push: unverified.
- **Scheduled tasks (formerly routines), cloud.** Research preview. Triggers: schedule, API, GitHub. An API trigger gives an endpoint `https://api.anthropic.com/v1/claude_code/routines/<id>/fire`; a POST with a bearer token "starts a new session and returns a session URL". It takes an optional freeform `text` field, wrapped so the task sees it as untrusted data. Beta header `experimental-cc-routine-2026-04-01`. Limit: 100 API fires per hour per account. The endpoint is for claude.ai users and not part of the Claude Platform API. GitHub triggers do not reuse sessions. URL: https://code.claude.com/docs/en/routines.
- **Claude Tag / Claude in Slack.** Starts cloud sessions from Slack mentions (Slack only). URLs: https://code.claude.com/docs/en/changelog and https://code.claude.com/docs/en/channels.
- **`watch_url`.** In this container the tool `mcp__Claude_Code_Remote__watch_url` creates an inbound webhook whose POST body is delivered as a `<webhook-payload>` message and wakes an idle session; it ends when the session ends (from the tool description in this session; I found no public doc page and did not call it).
- The brief mentions a `PushNotification` tool. I did not find one in the docs I read; mark as unverified.

---

## 3. Fit with a cloud session

- **Network levels.** None, Trusted (default), Full, Custom. Custom takes your own domain list, optionally with the defaults; a leading `*.` matches subdomains. The environment's changes reach running sessions in about a minute. URL: https://code.claude.com/docs/en/cloud-environments.
- **Default allowed domains.** The Trusted list I read has no `discord.com`, `gateway.discord.gg`, `cdn.discordapp.com`, `api.telegram.org`, Twilio, ntfy or Pushover entries (I searched the saved page for those names and found none). Notable entries: `api.anthropic.com`, `api.github.com`, `*.googleapis.com`, `*.amazonaws.com`, `*.modelcontextprotocol.io`. Source: same URL.
- **Hosts a Discord bot would need.** `discord.com` for the REST API (read: docs host https://docs.discord.com/developers/events/gateway). `gateway.discord.gg` for the Gateway (the exact host comes from the `/gateway/bot` response; `gateway.discord.gg` is the hostname I know from experience, unverified in a fetched page). `cdn.discordapp.com` for attachment downloads (unverified in a fetched page).
- **Websocket through the proxy.** Unverified. The proxy README says outbound HTTPS is tunnelled to an egress proxy with an allowlist and TLS re-terminated (read: /root/.ccr/README.md in this container). A `wss://` connection starts as an HTTPS CONNECT, so it may pass for an allowed host, but neither source states websocket support, and I did not test.
- **Idle and hibernation.** "After a few minutes without activity, a session's VM pauses with its files saved. Your next message restores the same VM." The setup script does not run on restore. A paused VM can be reclaimed. A Bash command waits at most 10 minutes foreground, then moves to the background for up to 30 more minutes. A paused VM cannot hold a Gateway websocket. URL: https://code.claude.com/docs/en/cloud-environments.
- **Secrets.** Environment variables are readable by anyone who uses the environment. A network secret is held by the agent proxy and attached to requests for listed hosts, as a custom header with an optional prefix, so the VM never sees the key. URL: https://code.claude.com/docs/en/cloud-environments. Inference: Discord REST expects `Authorization: Bot <token>`, which the header-with-prefix form could carry. The Gateway sends the token inside an Identify message, not in a header, so a network secret would not cover a Gateway connection from the VM.
- **Inbound routes that start or wake a cloud session.** Scheduled task schedule, scheduled task API `/fire` (new session each call), GitHub events (new session each event), `watch_url` (wakes the session that created it), and a message typed into the session from claude.ai, the mobile app or Slack. URLs: https://code.claude.com/docs/en/routines and the tool description above.

### Who holds the connection (options, no choice made)

1. **A small always-on relay outside the session** (a cheap VM, container service or Raspberry Pi). It holds the Discord Gateway (or the Telegram long poll or webhook) and checks the sender. On a message it calls `/fire` (new session, `text` field, 100 per hour) or a `watch_url` hook (wakes an existing session). The robot replies through the REST API or a send tool.
2. **Jim's own computer** running Claude Code with a channel plugin (Telegram or Discord) or Remote Control with push. The session runs locally, not as a cloud session. The computer must stay on and `claude` must keep running (read: https://code.claude.com/docs/en/remote-control).
3. **A hosted bot or connector.** A third-party hosted MCP server (for example a Smithery-hosted Discord server in the registry) or a claude.ai connector (Bird for WhatsApp and SMS, AnythingMCP for Telegram, per the brief) removes the relay for outbound messages. These sit between the session and the chat app and hold the bot token; none receives a reply and wakes a session in anything I read.
4. **Outbound only.** The robot sends alerts and Jim replies in the web or mobile Claude session; no relay needed.

---

## 4. Alternatives

For each: MCP, API and terms, inbound and outbound path, cost, fit for Jim.

### Telegram
- MCP: PyPI `telegram-mcp` lets an agent message a user and wait for a reply, defaulting to a hosted server (search summary: https://pypi.org/project/telegram-mcp). chaindead/telegram-mcp (Go, ~331 stars, updated 2026-05-28, user account via MTProto; directory listing, search summary: https://enterprisedna.co/directories/mcp/chaindead-telegram-mcp). FantomaSkaRus1/telegram-bot-mcp (174 Bot API tools, 3 stars; search summary). Registry entries include `ai.awraiter/telegram` (channel analytics) and `co.civai.nova/telegram-agent` (read: https://registry.modelcontextprotocol.io/v0/servers?search=telegram&limit=6). Anthropic ships a Telegram channel plugin (read: https://code.claude.com/docs/en/channels).
- API and terms: Bot API through BotFather (`/newbot`). Updates arrive by `getUpdates` long polling (earliest 100 unconfirmed) or a webhook on ports 443, 80, 88 or 8443; the two cannot be used at once. Read: https://core.telegram.org/bots/faq. The FAQ has no terms section. Userbot (personal account) servers carry more risk; I did not check Telegram's terms on them.
- Limits and cost: bots may message users "at no cost"; about 1 message per second per chat, 30 per second in bulk, 20 per minute per group; paid broadcasts cost 0.1 Stars past 30 per second. Read: same URL.
- Cloud fit: sending needs `api.telegram.org` (not in the Trusted list; Custom level). Receiving needs a poller or an HTTPS webhook receiver, so the relay pattern applies.
- Fit for Jim: strongest of the chat apps on cost and ease; separate from work Slack.

### Signal
- No official bot API found. signal-cli is an unofficial command line, JSON-RPC and D-Bus interface; signal-cli-rest-api wraps it in HTTP; a bot links as a secondary device on a phone number. Search summary only: https://github.com/topics/signal-cli. I found nothing on Signal's terms regarding this; unverified. No Signal MCP found in my searches.
- Fit: privacy-friendly, but unofficial tooling, a dedicated number, and an always-on host are needed.

### WhatsApp (Business Platform, Bird)
- Meta's Business Platform bills per delivered message since 1 July 2025; rates depend on category and country; a US figure of about $0.004 for utility and $0.025 for marketing comes from vendor blogs; some sources say service replies in the 24-hour window stop being free from 1 October 2026 (search summary; conflicting; Meta's page not read: https://developers.facebook.com/documentation/business-messaging/whatsapp/pricing). Needs a Meta business account, a phone number and templates for messages outside the 24-hour window (my understanding; not confirmed in a source I read).
- Bird sends WhatsApp and SMS through a claude.ai connector (brief; not re-checked).
- Fit: personal WhatsApp is Jim's channel to friends; business set-up is heavy and costs money. Not recommended on ease.

### Matrix
- MCP: `mcp-server-matrix` (PyPI, Baho73, MIT, matrix-nio; send, read, rooms, invites; works with Synapse, Dendrite, Conduit). Search summary: https://pypi.org/project/mcp-server-matrix/. Another Node server on agentpedia (https://agentpedia.codes/mcp/matrix; search summary).
- Open protocol, no per-platform bot approval, self-hostable; inbound needs a sync loop (a long-lived HTTP request) in a relay. Cost is a server or a hosted account. I did not read Matrix terms.
- Fit: good on control and separation, heavier to run.

### iMessage
- Anthropic ships an iMessage channel plugin. It reads `~/Library/Messages/chat.db` and replies through AppleScript; needs macOS, Full Disk Access, and the session on that Mac. Self-chat bypasses the allowlist; other senders are added by handle. Read: https://code.claude.com/docs/en/channels. Not usable from a cloud session (local Mac only).

### Microsoft Teams
- Not researched with a source; it is a work tool, so it conflicts with Jim's wish to keep work and personal separate. Unverified.

### Email
- This session has a Gmail connector with send, reply and read tools (tool list in this environment). Connector traffic goes through Anthropic's servers and needs no allowlist entry (read: https://code.claude.com/docs/en/cloud-environments). Inbound email does not wake a session; a relay or a scheduled task that reads the inbox would. Cost: free. Terms: not checked. Slow for conversation, but available now.

### SMS (Twilio)
- US outbound about $0.0079 per segment and inbound $0.0075, plus carrier fees (about $0.003 to $0.01, third-party estimate), plus 10DLC registration: brand $4.50 (sole proprietor) or $46, campaign vetting $15 one-time, $2 a month (starter) or $10 a month (standard). Search summary citing Twilio's pages: https://www.twilio.com/en-us/sms/pricing/usa and https://support.twilio.com/hc/en-us/articles/1260803965530. Inbound SMS is a webhook to a public URL, so a relay is needed. No official MCP checked.
- Fit: works on any phone; registration and per-message cost make it the poorest value for a hobby use.

### Push notification apps
- ntfy: publish with `curl -d "text" ntfy.sh/topic`; topic name is effectively a password; up to three action buttons per notification, which can send an HTTP request (read: https://docs.ntfy.sh/publish/). No free-text reply path found. Hosted free tier; paid plans about $5, $10, $20 a month; self-host under Apache 2.0; iOS push through a self-hosted server relays via ntfy.sh (search summary: https://railway.com/deploy/ntfy-1.md). Authentication and rate-limit sections were not read.
- Pushover: $4.99 one-time per platform; 10,000 messages a month per account free; extra capacity $50 for 10,000 (search summary of https://pushover.net/pricing and https://support.pushover.net/i12-message-size-and-frequency-limitations; per-account versus per-application wording changed, with a blog dating the change to 1 May 2026). One-way unless paired with something else.
- Fit: best cheap alert channel; replies need a second path.

### Slack (reference)
- Claude Tag and Claude in Slack start cloud sessions from mentions and post back (read: https://code.claude.com/docs/en/changelog and https://code.claude.com/docs/en/slack, the latter listed in https://code.claude.com/docs/llms.txt). This is the only chat app with a documented path into cloud sessions, and it is Jim's work tool.

### Claude connectors (claude.ai registry)
- Per the brief, no Discord connector; AnythingMCP lists Telegram among 180+ APIs; Bird sends WhatsApp and SMS. I did not re-run the registry search. Connector traffic bypasses the network allowlist.

---

## 5. Security

- **Inbound command channel.** Anthropic's own guidance: "An ungated channel is a prompt injection vector." Check the sender before emitting anything, gate on the sender ID and not the room ID, because in group chats anyone in an allowlisted room could inject messages. Read: https://code.claude.com/docs/en/channels-reference.
- **Sender authentication in Anthropic's plugins.** Pairing: DM the bot, receive a code, approve it in the session, then set policy `allowlist` so strangers get no pairing replies. Others are silently dropped. Read: https://code.claude.com/docs/en/channels and the Discord plugin page above. The sender is authenticated only by the platform's account ID, so Jim's Discord or Telegram account security is the root of trust.
- **Permission relay widens the risk.** A reply of `yes <id>` can approve a Bash call; five random letters, an ID cannot be reused, and only allowlisted senders count. Relay text is sanitised from v2.1.211 and credentials with known prefixes are masked from v2.1.234; secrets without a recognisable prefix reach the chat unmasked. Read: https://code.claude.com/docs/en/channels-reference.
- **Fire text.** The `/fire` `text` arrives wrapped as untrusted data, and a saved prompt must opt in to act on it; anyone with the bearer token can send text. Read: https://code.claude.com/docs/en/routines. A relay that forwards Jim's reply text this way keeps that wrapper, but the relay's own token then becomes a credential that can start sessions.
- **Message content as injection.** Anything a robot reads from a shared channel (other members, link previews, attachments, `fetch_messages` history) is untrusted input. Plugins do not download attachments automatically (read: Discord plugin page). In a DM-only server with one human member and the bot, the only untrusted text is Jim's own and what he pastes.
- **Token storage.** Plugins write the bot token to `~/.claude/channels/discord/.env` (read: https://code.claude.com/docs/en/channels). In a cloud environment, environment variables are readable by everyone who uses the environment, and a network secret keeps a header-borne key outside the VM (read: https://code.claude.com/docs/en/cloud-environments). A Gateway token cannot use a network secret (inference above), so a relay outside the VM is the place for that token.
- **Blast radius of a leaked bot token.** The token lets the holder act as the bot in every server it joined, within the permissions granted. With the plugin's list (View Channels, Send Messages, Send Messages in Threads, Read Message History, Attach Files, Add Reactions) that means reading history of the channels it can see and posting as the bot, and DMing users who share a server (read: https://code.claude.com/docs/en/channels). The token can be reset in the Developer Portal. A bot in one private server holding only these permissions limits the exposure. Discord's own incident handling for leaked tokens: not researched.
- **Account risk.** Using a user token for automation risks termination of Jim's personal account (search summary above).

---

## Gaps

- The Discord Developer Policy text (all routes returned 403 or redirected to a 403): exact wording on automation, message content and AI use not quoted.
- Whether a bot can open a DM with a user it shares no server with.
- Whether websockets pass the cloud agent proxy.
- Whether channels work in cloud sessions (docs silent; my reading is that they do not).
- Whether a cloud session can send a mobile push, and whether a `PushNotification` tool exists.
- Signal and Teams terms; Matrix terms; Telegram terms of service text; WhatsApp current rate card (Meta page not read).
- Hacker News threads on agents controlled through chat apps: not searched.
- Changelog entries from earlier in 2026 (the changelog page read only versions 2.1.300 to 2.1.314).
- Whether modelcontextprotocol/servers lists a Discord server.

## References

| URL | Status |
|---|---|
| https://code.claude.com/docs/en/channels | read |
| https://code.claude.com/docs/en/channels-reference | read |
| https://code.claude.com/docs/en/cloud-environments | read (summary, then grep of the saved page) |
| https://code.claude.com/docs/en/remote-control | read (grep of saved page) |
| https://code.claude.com/docs/en/routines | read (grep of saved page) |
| https://code.claude.com/docs/en/whats-new/2026-w16 | read |
| https://code.claude.com/docs/en/changelog | read in part (first 100,000 of 1,141,410 characters) |
| https://code.claude.com/docs/llms.txt | read (grep) |
| https://code.claude.com/docs/en/slack | listed in llms.txt; not read |
| https://docs.discord.com/developers/events/gateway | read |
| https://docs.discord.com/developers/topics/rate-limits | read |
| https://docs.discord.com/developers/resources/user | read |
| https://discord.com/developers/docs/policies-and-agreements/developer-policy | failed (301 redirect) |
| https://docs.discord.com/developers/policies-and-agreements/developer-policy | failed (307 redirect) |
| https://support-dev.discord.com/hc/articles/8563934450327-Discord-Developer-Policy | failed (403) |
| https://support-dev.discord.com/hc/en-us/articles/8563934450327-Discord-Developer-Policy | failed (403) |
| https://support-dev.discord.com/hc/articles/25280499088279 | search summary |
| https://support.discord.com/hc/articles/115002192352 | failed (403); search summary |
| https://discord.com/terms/guidelines | search summary |
| https://discord.com/safety/important-policy-updates | search summary |
| https://github.com/anthropics/claude-plugins-official/tree/main/external_plugins/discord | read (summariser) |
| https://github.com/anthropics/claude-plugins-official/tree/main/external_plugins/telegram | listed only; not read |
| https://github.com/SaseQ/discord-mcp | read (summariser) |
| https://enterprisedna.co/directories/mcp/saseq-discord-mcp | search summary |
| https://registry.modelcontextprotocol.io/v0/servers?search=discord&limit=10 | read (output cut off) |
| https://registry.modelcontextprotocol.io/v0/servers?search=telegram&limit=6 | read (output cut off) |
| https://implicator.ai/anthropic-ships-its-openclaw-rival-connecting-claude-code-to-telegram-and-discord | search summary |
| https://github.com/python-discord/bot/issues/1640 | search summary |
| https://core.telegram.org/bots/faq | read |
| https://pypi.org/project/telegram-mcp | search summary |
| https://enterprisedna.co/directories/mcp/chaindead-telegram-mcp | search summary |
| https://github.com/topics/signal-cli | search summary |
| https://pypi.org/project/mcp-server-matrix/ | search summary |
| https://agentpedia.codes/mcp/matrix | search summary |
| https://developers.facebook.com/documentation/business-messaging/whatsapp/pricing | search summary (page not read) |
| https://www.twilio.com/en-us/sms/pricing/usa | search summary |
| https://support.twilio.com/hc/en-us/articles/1260803965530 | search summary |
| https://docs.ntfy.sh/publish/ | read in part (first 100,000 of 205,885 characters) |
| https://railway.com/deploy/ntfy-1.md | search summary |
| https://pushover.net/pricing | search summary |
| https://support.pushover.net/i12-message-size-and-frequency-limitations | search summary |
| /root/.ccr/README.md | read (local file) |
