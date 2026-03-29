import{a as l,j as e}from"./vendor-C5RdOJEA.js";import A from"./PublicNav-I-L7Bvzf.js";import C from"./PublicFooter-utP_VqAg.js";import{A as T}from"./AuraLogo-hFFSPhKT.js";import{u as B}from"./usePageTitle-8jQ9V7Sz.js";import{f as I}from"./pricing-BciwUaSB.js";const P="_page_1r8wr_4",_="_hero_1r8wr_11",E="_heroLogoWrap_1r8wr_18",x="_heroTitle_1r8wr_22",R="_heroSub_1r8wr_29",q="_contentWrap_1r8wr_37",N="_sidebar_1r8wr_47",L="_sidebarBtn_1r8wr_54",G="_sidebarBtnActive_1r8wr_74 _sidebarBtn_1r8wr_54",M="_contentArea_1r8wr_86",O="_sectionTitle_1r8wr_91",K="_docCard_1r8wr_102",j="_docCardHeader_1r8wr_110",D="_docCardTitle_1r8wr_122",F="_docCardChevron_1r8wr_128",V="_docCardChevronOpen_1r8wr_134 _docCardChevron_1r8wr_128",W="_docCardBody_1r8wr_139",U="_docCardBodyContent_1r8wr_143",H="_ctaSection_1r8wr_154",Y="_ctaTitle_1r8wr_160",z="_ctaSub_1r8wr_167",Q="_ctaLink_1r8wr_173",X="_faqSearchWrap_1r8wr_189",$="_faqSearchIcon_1r8wr_200",Z="_faqSearch_1r8wr_189",J="_faqSearchClear_1r8wr_220",ee="_faqNoResults_1r8wr_234",te="_mobileNavToggle_1r8wr_242",ae="_mobileNavBtn_1r8wr_250",oe="_mobileNavChevron_1r8wr_267",re="_mobileNavDropdown_1r8wr_273",se="_mobileNavItem_1r8wr_286",ie="_mobileNavItemActive_1r8wr_306 _mobileNavItem_1r8wr_286",t={page:P,hero:_,heroLogoWrap:E,heroTitle:x,heroSub:R,contentWrap:q,sidebar:N,sidebarBtn:L,sidebarBtnActive:G,contentArea:M,sectionTitle:O,docCard:K,docCardHeader:j,docCardTitle:D,docCardChevron:F,docCardChevronOpen:V,docCardBody:W,docCardBodyContent:U,ctaSection:H,ctaTitle:Y,ctaSub:z,ctaLink:Q,faqSearchWrap:X,faqSearchIcon:$,faqSearch:Z,faqSearchClear:J,faqNoResults:ee,mobileNavToggle:te,mobileNavBtn:ae,mobileNavChevron:oe,mobileNavDropdown:re,mobileNavItem:se,mobileNavItemActive:ie},c=[{id:"getting-started",title:"Getting Started",icon:"&#128640;",items:[{title:"What Is Aura Alpha?",content:`Aura Alpha is a multi-bot, risk-siloed algorithmic trading platform. It runs up to 3 independent trading bots (Alpha, Beta, Crypto) against 83 pre-built strategies across US equities and crypto markets.

Key concepts:
• Bots — Isolated agents. Each has its own IBKR clientId, capital allocation, and strategy assignments.
• Strategies — Codified entry/exit rules. You choose which strategies a bot runs.
• Kill Switch — Master override that stops all trading instantly (POST /api/control/kill).
• Paper Trading — All strategies can run in paper (simulated) mode before going live.
• Signal Validation — Every signal is logged with a UUID, confidence score, and backtest stats.`},{title:"Create Your Account",content:I},{title:"Connect Interactive Brokers (IBKR)",content:`IBKR is the primary broker for equity trading. Your credentials never leave your machine.

1. Download IB Gateway (not TWS) from interactivebrokers.com.
2. Launch IB Gateway and sign in. Complete 2FA when prompted.
3. In Configure > Settings > API:
   - Set Socket port to 4001
   - Enable "Allow connections from localhost only"
   - Check "Bypass Order Precautions for API Orders" for automation
4. Assign clientIds in Bot Management:
   - Alpha → clientId 1
   - Beta  → clientId 2
   - Crypto → clientId 3
5. Verify in System Health — Gateway Status should show "Connected".

Note: IBKR sessions expire after ~24 hours. 2FA is required on every restart.`},{title:"Connect Alpaca (No IBKR Required)",content:`Alpaca provides a free REST API for US equities. Paper trading available with no funding.

1. Create a free account at alpaca.markets.
2. Generate API keys under API Keys in the Alpaca dashboard.
3. In Aura Alpha: Settings > Brokers > Add Broker Connection.
4. Select Alpaca, paste your API key and secret, choose Paper or Live.
5. Assign to a bot in Bot Management > Edit Bot > Broker.

Alpaca paper trading uses a simulated $100,000 account with instant fills at last trade price.`},{title:"Enable Paper Trading",content:`Paper trading simulates trades using real-time prices without real orders.

To enable:
1. In Bot Management, find the bot you want.
2. Click Edit Bot and toggle Paper Trading: ON.
3. Save.

The bot routes all orders to the paper engine. Paper fills use last OHLCV close + 0.05% simulated slippage.

To switch back to live: toggle Paper Trading: OFF, then re-run the strategy promotion workflow (live promotions are required before the bot trades real capital).`},{title:"Run Your First Backtest",content:`The backtest engine uses Polars and DuckDB — processes millions of OHLCV bars in seconds.

1. Navigate to Backtest Dashboard.
2. Select a strategy (try "Breakout" or "Bullish Pullback" to start).
3. Set date range (2-year default) and universe (US Tier 1 = 200 large-cap symbols).
4. Click Run Backtest. Watch progress in real time.
5. Review results:
   - Win rate (aim for >45%)
   - Profit factor (aim for >1.5)
   - Max drawdown (watch for >20%)
   - Sharpe ratio (aim for >1.0)
   - Trade count (need >30 for statistical relevance)
6. Click Promote to Paper when results look solid.`},{title:"Understanding the Dashboard",content:`The Command Center (home page) shows:

• Bot Status — Alpha, Beta, Crypto heartbeat status. Green = active, red = stopped.
• Live Positions — All open positions across all bots, aggregated.
• Today's P&L — Intraday realized + unrealized P&L.
• Signal Feed — Real-time signals from the dual-loop scanner (60s fast / 300s full).
• Kill Switch — Large red button top-right. One click, confirm, stops everything.
• System Health — Gateway connectivity, data freshness, API latency.

The dashboard auto-refreshes every 30 seconds. Kill switch and portfolio brain panels update every 5 seconds.`}]},{id:"strategies",title:"Strategies",icon:"&#9881;",items:[{title:"The 83-Strategy Catalogue",content:`Aura Alpha includes 83 pre-built strategies in its catalogue:

Long Strategies (52+):
• The 5 Day Bounce — Buys pullback after 5 consecutive down days in an uptrend
• Breakout — Enters on multi-day resistance break with above-average volume
• Bullish Pullback — Mean-reversion entries in stocks above their 50 SMA
• Trend Play — EMA crossover momentum entry (9/21 EMA alignment)
• Nice Chart — Pattern-based: clean range, orderly pullback, coiled price action
• Staggering Volume — Relative volume spike >3x 20-day average
• Power Hour Long — 3:00–4:00 PM ET end-of-day momentum play
• Sunrise Mover — Pre-market gap-up continuation play
• Horseshoe Up — U-shaped recovery pattern entry
• AVWAP Bounce — Entry on anchored VWAP reclaim (post-earnings/catalyst)

Short Strategies (21+):
• Cuts Like a Knife — Breakdown entry on failed support level
• Topping Formation — Head-and-shoulders / double-top with volume confirmation
• Downward Dog — Extended downtrend continuation play
• Slippery Slope — Ascending wedge breakdown
• Bon Shorty — Failed earnings gap fade
• Failed Bounce — Dead cat bounce failure at resistance

Crypto Bot:
• 79 Kraken pairs, Gemini Flash AI scoring, 10 concurrent positions, runs 24/7`},{title:"How Signals Are Generated",content:`The signal engine runs two parallel loops:
• Fast loop — 50 priority symbols every 60 seconds (symbols in open positions or watchlist)
• Full loop — All universe symbols every 300 seconds

Each loop evaluates:
• Moving averages: 9 EMA, 21 EMA, 50 SMA, 200 SMA
• Volume: Relative volume vs. 20-day avg, VWAP deviation, AVWAP
• Momentum: RSI, ADX, MACD
• Patterns: Breakouts, pullbacks, wedges, gaps
• Time filters: Market hours (9:30–4:00 PM ET), power hour, opening range

Every signal logged to signal_log with:
• UUID for traceability
• Strategy name and version hash
• Confidence score (0.0–1.0; bots require ≥0.50 to act)
• Backtest win rate and Sharpe at time of signal

View live signals: GET /api/realtime/signals
Stream via SSE: GET /api/realtime/stream`},{title:"Strategy Promotion Workflow",content:`Strategies follow a strict pipeline — you cannot skip steps:

  Backtest → Paper Trading → Operator Approval → Live

Step 1: Promote to Paper
After a successful backtest, click "Promote to Paper." The strategy generates live simulated trades.

Step 2: Validate Paper Results (min. 2 weeks recommended)
• Is the live hit rate close to backtest hit rate? (>5% divergence = investigate)
• Is the strategy triggering at expected frequency?
• Are entries/exits at reasonable prices?

Step 3: Request Live Approval
Navigate to Strategy Promotion and click "Request Live Approval." An admin must sign off. This is audited.

Step 4: Live
Strategy status changes to LIVE. Bot places real orders.`},{title:"Suppressing Underperformers",content:`To suppress a live strategy:
1. Go to Strategy Catalogue and find the strategy.
2. Click Suppress (requires confirmation).
3. Status changes to SUPPRESSED. Bot stops generating new signals from it.
4. Existing open positions are NOT automatically closed.

To re-activate: click Unsuppress and re-run the approval workflow.

The platform never auto-suppresses a strategy without your explicit action.`},{title:"Strategy Versioning and Hash System",content:`Every strategy definition is versioned with a SHA-256 hash of its rule set. When rules change, a new hash is generated and the old version is archived.

This means:
• Backtest results are always tied to a specific strategy version hash.
• If you update parameters, you must re-backtest before promoting.
• The signal_log records the version hash for each signal — auditable proof that a strategy was not modified between signal and outcome.

View strategy versions: GET /api/strategies/{name}`},{title:"Understanding Hit Rate, Sharpe, and Drawdown",content:`Hit Rate (Win Rate):
% of trades that closed at a profit. A 55% hit rate on 1:1 risk/reward is mildly positive. Adjust expectations based on the risk/reward ratio.

Sharpe Ratio:
(mean_return - risk_free_rate) / std_dev_return
• < 0: Worse than cash
• 0–1: Acceptable
• 1–2: Good
• >2: Excellent
All Aura Alpha Sharpe calcs use daily granularity, 0% risk-free rate.

Max Drawdown:
Largest % decline from a strategy's equity peak to its subsequent trough. Anything above 30% warrants careful review before live deployment.

Profit Factor:
Gross Profit / Gross Loss. >1.5 is healthy; >2.0 is strong. Below 1.0 means the strategy loses money on aggregate.

Confidence Score:
Composite 0–1 score: 40% ML probability + 30% backtest Sharpe + 20% rolling live hit rate + 10% liquidity quality. Minimum 0.50 required for bot action.`},{title:"ML Scoring and Edge Decay",content:`The ML model retrains every Sunday at 2 AM UTC:
1. Collects signal/outcome pairs from trailing 90 days.
2. Trains gradient-boosted model on: strategy, symbol, regime, time, volume, RSI, etc.
3. Outputs new confidence weights per strategy-symbol.
4. Bots pick up new scores on their next scan cycle.

Edge decay report runs every Sunday at 11 PM UTC:
• Flags strategies whose rolling 30-day live hit rate dropped >10% below backtest baseline.
• Sends ntfy push notification.
• Marked as DECAYING in the Strategy Catalogue.

View ML scores: GET /api/ml/analytics
Trigger manual retrain (admin): POST /api/ml-ops/retrain`}]},{id:"risk",title:"Risk Management",icon:"&#128737;",items:[{title:"The Kill Switch",content:`The kill switch is the most important safety control. When activated, it:
1. Sets kill=true in state/kill_switch.json globally.
2. Publishes kill event to Redis pub/sub (<100ms propagation to all workers).
3. Prevents all bots from submitting new orders.
4. Does NOT automatically close open positions (intentional — rushed exits cause worse fills).

To activate:
• Dashboard: Click the red Kill Switch button (top-right), type CONFIRM, click red button.
• API: POST /api/control/kill with {"reason": "manual halt"}
• Mobile (ntfy): Send kill command to topic aura-alerts-3de78154

To reset: Control Plane > Reset Kill Switch > Confirm.

The kill switch state is always visible in the top nav bar — red indicator = active.`},{title:"The 7-Layer Bot Safety System",content:`Bots only trade when ALL 7 conditions are met. If any fails, the bot logs the block reason and skips submission.

Layer 1: Kill switch — must be inactive
Layer 2: Heartbeat — bot must have sent a heartbeat within the last 5 minutes
Layer 3: Equity data — bot must be able to read its account equity from IBKR
Layer 4: Strategy approval — strategy must be in LIVE status (explicit promotion required)
Layer 5: Confidence threshold — signal confidence must be ≥0.50 (configurable)
Layer 6: Liquidity filter — symbol's 20-day avg volume must be ≥10k shares (configurable)
Layer 7: Risk regime — market must not be in cautious mode (unless bot is allowed in cautious)

View current health gate status: GET /api/control/health`},{title:"VaR and CVaR Explained",content:`Value at Risk (VaR) answers: "How much could I lose on a bad day?"

95% 1-day VaR of $1,000 means: "On 95 out of 100 trading days, I expect to lose no more than $1,000."

Conditional VaR (CVaR) answers: "On the bad days that exceed VaR, how much do I actually lose on average?"
CVaR is always worse than VaR. If VaR=$1,000 at 95%, CVaR might be $2,500.

How Aura Alpha calculates VaR:
• 252 trading days of historical returns per position
• Historical simulation method (5th percentile of ranked daily returns)
• Aggregated across all positions using correlation matrix (correlated positions aren't fully diversifying)
• Recalculated every 5 minutes during market hours

Practical rule: if portfolio 95% VaR exceeds 2% of total account equity, consider reducing position sizes.

View current VaR: GET /api/risk/var`},{title:"Risk Siloing Between Bots",content:`Alpha, Beta, and Crypto bots are fully isolated:

1. Separate IBKR clientIds — Each bot gets a unique client ID. IBKR treats them as separate sessions.
2. Separate capital allocations — Each bot has its own max allocation percentage.
3. No cross-bot position sharing — A position opened by Alpha cannot be closed by Beta.
4. Portfolio Brain is read-only — Cross-bot brain monitors combined exposure and flags conflicts (e.g., Alpha long XYZ while Beta short XYZ) but NEVER automatically resolves them.

Portfolio Brain snapshot every 5 minutes during market hours.
Force fresh snapshot: POST /api/portfolio-brain/snapshot`},{title:"Position Limits",content:`Each bot enforces:
• Max single position size — Default: 10% of allocated capital (configurable)
• Max concurrent positions — Alpha and Beta: configurable (default 5), Crypto: 10
• Max sector exposure — Default: 30% max in any single GICS sector
• Max crypto exposure (Crypto bot) — 95% of allocated capital

Position sizing uses Kelly Criterion adjusted by the strategy's backtest Sharpe ratio, capped at the per-bot maximum. High-confidence, high-Sharpe strategies get larger positions automatically.

View current exposure: GET /api/risk/exposure`},{title:"Cautious Mode",content:`Cautious mode is activated when the regime detector identifies elevated risk.

In cautious mode:
• Position sizes are halved (0.5× Kelly multiplier)
• Strategies with Sharpe <1.0 are paused
• Only top 10 highest-confidence signals per bot are acted on
• Crypto bot's exposure cap drops from 95% to 60%

Cautious mode triggers when ANY of these are true:
• VIX > 30
• 14-day realized volatility of SPY > 25%
• Portfolio drawdown from peak > 15%

Regime detector runs every 5 minutes. Market intelligence state: state/market_intelligence.json

Force cautious mode: POST /api/control/regime {"mode": "cautious"}
Return to normal: POST /api/control/regime {"mode": "normal"}`},{title:"Drawdown Monitoring",content:`The platform tracks drawdown at three levels:

1. Strategy-level — Per-strategy equity curve, backtest max drawdown, rolling 30-day live drawdown.
2. Bot-level — Aggregate P&L for all positions managed by a bot, from the bot's inception high.
3. Portfolio-level — Combined P&L across all bots.

Configurable drawdown alerts:
• Warning threshold — Alert when drawdown exceeds X% (default: 10%)
• Halt threshold — Automatically suppress all new signals when drawdown exceeds Y% (default: 20%)

View current drawdown: GET /api/risk/drawdown`}]},{id:"brokers",title:"Brokers",icon:"&#127968;",items:[{title:"Supported Brokers",content:`Official API Brokers (Full Support):
• Interactive Brokers (IBKR) — Equities, Options, Futures, Forex, Bonds. Live + Paper.
• Alpaca — US Equities, Crypto. Live + Paper.
• Kraken — Crypto (79 pairs). Live only.
• Coinbase Advanced — Crypto. Live only.
• Binance US — Crypto. Live only.
• Bybit — Crypto, Futures. Live + Paper.
• Tastytrade — Equities, Options. Live only.
• TradeStation — Equities, Futures. Live + Paper.

Unofficial API Brokers (use at your own risk):
• Webull — Unofficial client. Prone to breaking on app updates.
• Robinhood — Unofficial. Actively blocks API access periodically.

DISCLAIMER: Using unofficial APIs may violate the broker's Terms of Service. Aura Alpha does not endorse this and accepts no responsibility for account restrictions.`},{title:"Connecting IBKR — Step by Step",content:`Prerequisites: Approved IBKR account, IBKR Secure Login System (SLS) enabled.

1. Download IB Gateway (not TWS) from interactivebrokers.com.
2. Launch and sign in. Complete 2FA (mandatory every session).
3. Configure > Settings > API:
   - Socket port: 4001
   - Enable "Allow connections from localhost only"
   - Enable "Bypass Order Precautions for API Orders"
4. Ensure AllowBlindTrading=yes in ~/.TWS/jts.ini (Linux) or C:Jtsjts.ini (Windows).
5. Assign clientIds in Bot Management for each of your bots.
6. Verify: System Health > Gateway Status should show "Connected."

IBKR sessions expire after ~24 hours. 2FA is required on every restart. The autorepair cron auto-reconnects within 60 seconds.

Note: for complex/leveraged ETFs, request "Complex or Leveraged ETF" permissions in IBKR's account management portal.`},{title:"Connecting Alpaca — Step by Step",content:`1. Create a free account at alpaca.markets.
2. Generate API keys: Dashboard > API Keys > Generate New Key.
   Copy both the API Key ID and Secret Key (secret shown once only).
   Paper trading uses separate paper environment keys.
3. In Aura Alpha: Settings > Brokers > Add Broker Connection > Alpaca.
4. Paste API Key ID and Secret Key, select Paper or Live.
5. In Bot Management: Edit Bot > Broker > Select your Alpaca connection > Save.

Alpaca paper uses a simulated $100,000 account with instant fills at last trade price.
Alpaca live routes to real US equity markets during regular session hours (9:30 AM–4:00 PM ET).`},{title:"Connecting Kraken (Crypto Bot)",content:`1. In Kraken: Security > API > Create API Key.
   Grant permissions: Query Funds, Query Open Orders & Trades, Create & Modify Orders, Cancel Orders.
   Do NOT grant Withdraw Funds.
2. In Aura Alpha: Settings > Brokers > Add Broker Connection > Kraken.
3. Paste your API key and private key.
4. In Bot Management: Edit Bot (Crypto) > Broker > Select Kraken > Save.

The Crypto bot begins scanning 79 Kraken pairs on its next full scan cycle (within 300 seconds).`},{title:"Switching Brokers",content:`You can switch a bot's broker connection at any time, but:
• Switching does NOT transfer open positions. Positions opened with Broker A remain there.
• After switching, the bot routes new orders to Broker B.
• Switching from IBKR to Alpaca releases the IBKR clientId.

To switch: Bot Management > Edit Bot > Broker > Select new broker > Save.`},{title:"API Key Security Best Practices",content:`1. Never share API keys. Each bot should have its own key with minimum required permissions.
2. Use IP whitelisting where supported (Kraken, Alpaca both support this). Whitelist your server's IP only.
3. Never grant withdrawal permissions to trading API keys. Only query + order permissions are needed.
4. Rotate keys quarterly. Revoke old keys immediately if you suspect compromise.
5. IBKR Gateway is local — your IBKR credentials are entered directly into IB Gateway and never seen by Aura Alpha's servers.`}]},{id:"api",title:"API Reference",icon:"&#128187;",items:[{title:"API Overview",content:`The Aura Alpha Control Plane API runs on FastAPI (port 8020). All endpoints are under /api/.

Authentication: Bearer token in the Authorization header.
Base URL (production): https://auraalpha.cc/api/
Base URL (local): http://localhost:8020/api/

OpenAPI docs: https://auraalpha.cc/docs (when AURA_DOCS=on)`},{title:"Core Trading Endpoints",content:`Telemetry:
  GET  /api/telemetry/latest         — latest bot heartbeats
  POST /api/telemetry/heartbeat      — register heartbeat

Control:
  GET  /api/control/health           — health gate status (all 7 layers)
  POST /api/control/kill             — emergency kill switch
  GET  /api/control/approvals        — pending strategy approvals

Positions & Orders:
  GET  /api/positions                — current positions (all bots)
  GET  /api/orders                   — open orders
  GET  /api/orders/blotter           — full order blotter

Risk:
  GET  /api/risk/exposure            — portfolio exposure
  GET  /api/risk/correlation         — position correlation matrix
  GET  /api/risk/drawdown            — drawdown metrics
  GET  /api/risk/var                 — Value at Risk (95% + 99%)
  GET  /api/risk/margin              — margin utilization`},{title:"Signal Validation Endpoints",content:`Track Record:
  GET /api/track-record/signals       — all signals with UUID + confidence
  GET /api/track-record/outcomes      — closed positions matched to signals
  GET /api/track-record/reconciliation — daily drift analysis
  GET /api/track-record/summary       — aggregate performance
  GET /api/track-record/public        — public /results page data`},{title:"Real-Time Signal Endpoints",content:`Real-Time:
  GET /api/realtime/status           — scan loop health (fast + full loops)
  GET /api/realtime/signals          — latest generated signals
  GET /api/realtime/stream           — SSE streaming (Server-Sent Events)
  GET /api/realtime/priority         — priority symbol queue

Dual-loop: 60s fast scan (50 priority symbols) + 300s full scan.`},{title:"Portfolio Brain Endpoints",content:`Portfolio Brain:
  GET  /api/portfolio-brain/snapshot    — cross-bot positions snapshot
  GET  /api/portfolio-brain/conflicts   — conflict detection (e.g., opposing positions)
  GET  /api/portfolio-brain/exposure    — combined exposure analysis
  GET  /api/portfolio-brain/sectors     — sector breakdown across bots
  GET  /api/portfolio-brain/suggestions — allocation suggestions (suggest-only)
  POST /api/portfolio-brain/rebalance   — returns suggestion only — NO auto-execution`},{title:"Strategy and ML Endpoints",content:`Strategies:
  GET  /api/strategies                  — full catalogue list
  GET  /api/strategies/{name}           — strategy detail + version hash
  POST /api/strategies/{name}/promote   — promote to next stage
  POST /api/strategies/{name}/suppress  — suppress strategy

ML:
  GET  /api/ml/analytics                — current ML scores
  POST /api/ml-ops/retrain              — trigger ML retrain (admin)
  GET  /api/ensemble/scores             — ensemble signal scores

Backtests:
  GET  /api/backtest/results            — latest results
  GET  /api/backtest/status             — engine status / progress
  GET  /api/backtest/strategies         — strategy list with last backtest`}]},{id:"architecture",title:"Architecture",icon:"&#127959;",items:[{title:"System Architecture",content:`Aura Alpha is a multi-service platform:

• Control Plane API — FastAPI on port 8020 (165+ route files)
• Frontend — Vite + React on port 8181, proxies /api → localhost:8020
• IBKR Gateway — port 4001 (IB Gateway, always running)
• OHLCV Cacher — clientId 99, daemon + cron modes
• PostgreSQL — Primary database (53+ tables, 3.6M+ rows, migrated 2026-03-20)
• Redis — Rate limiting + kill switch pub/sub (<100ms propagation) — feature-flagged

Bots:
• Alpha — equity day-trading
• Beta  — equity day/swing-trading
• Crypto — crypto 24/7 (AI scoring)`},{title:"Data Stack",content:`• Parquet files — OHLCV data stored as Polars-compatible parquet
  - 500+ US symbols (3 tiers), 14 regions
  - data/ibkr/raw/us/, europe/, asia/, canada/, emerging/

• PostgreSQL — Transactional data (positions, orders, signals, audit log)
• DuckDB — Query engine for analytical/backtest queries
• Polars — DataFrame operations (faster than pandas at scale)
• YAML — Region configuration (data/ibkr/regions.yaml)

Data flow:
IBKR Gateway → OHLCV Cacher → Parquet files → Backtest Engine → Results → PostgreSQL`},{title:"Security Model",content:`• All API calls require Bearer token authentication (except /api/docs and public endpoints)
• Broker credentials stay on your local machine (IBKR Gateway) — never seen by Aura Alpha servers
• No trading data sent to external services
• AI Assistant calls are opt-in with a separate token budget
• Kill switch requires explicit confirmation (double-confirm in UI)
• Strategy promotion requires operator approval (audited)
• Full audit log of all actions (GET /api/audit)
• Redis kill switch pub/sub: <100ms global propagation`},{title:"Cron Schedule (EC2 Production)",content:`Every 2 min  — Autorepair (ops/ec2_autorepair.sh) — restarts crashed bots
Every 5 min  — Portfolio snapshot (scripts/portfolio_snapshot.py) — market hours only
Every 15 min — ntfy notifications (ops/alert_monitor.sh)
6 PM ET M-F  — Signal reconciliation (scripts/daily_reconciliation.py)
Sun 2 AM UTC — ML retrain (scripts/ml_retrain_pipeline.py)
Sun 11 PM UTC — Edge stability report (scripts/edge_decay_check.py)
3 AM UTC daily — PostgreSQL backup (ops/backup_db.sh)`}]}],w=[{q:"Why isn't my bot trading?",a:`Work through the 7-layer safety checklist:

1. Kill switch active? Check top nav bar — red = kill switch on. Reset at Control Plane > Reset Kill Switch.
2. No heartbeat? Check Bot Management — "Last Heartbeat" must be < 5 minutes ago. If stale, check the bot service status.
3. No equity data? IBKR Gateway may be disconnected. Check System Health > Gateway Status.
4. Strategy not approved? Strategies must show "Live" in Strategy Catalogue — not Paper, Backtested, or Suppressed.
5. Confidence threshold not met? Signals with confidence <0.50 are skipped. Expected behavior in choppy markets.
6. Liquidity filter? Symbols with 20-day avg volume <10k shares are skipped.
7. Cautious mode? Check Risk Management > Regime Indicator.`},{q:"What is the Crypto bot?",a:`The Crypto bot is Aura Alpha's digital asset trading agent. Key differences:
• Asset class: Crypto only, via Kraken (79 trading pairs)
• Hours: 24/7 including weekends and holidays
• AI scoring: Uses Gemini Flash to evaluate each signal
• Max concurrent positions: 10 (vs. 5 for equity bots)
• Max exposure: 95% of allocated capital

The Crypto bot is designed for 24/7 coverage without manual monitoring.`},{q:"What is the difference between Alpha and Beta bots?",a:`Both are equity trading bots using the same Aura Alpha strategy catalogue. Each has its own client ID, capital allocation, and configuration.

Many operators assign Alpha conservative strategies and Beta more aggressive ones, or run them on different symbol universes to diversify signal sources. They are fully isolated and can hold opposing positions in the same symbol (Portfolio Brain will flag this for your review).`},{q:"How do I add a symbol to my watchlist?",a:`1. Navigate to Watchlists in the sidebar.
2. Select an existing watchlist or click New Watchlist.
3. Click Add Symbol, type the ticker (e.g., AAPL, BTC/USD).
4. Click Add.

Watchlisted symbols join the fast scan loop (60s instead of 300s full scan) — you'll see signals faster.

Via API:
  POST /api/watchlists/{id}/symbols
  Body: {"symbol": "AAPL"}`},{q:"How often do strategies get scored by the ML model?",a:`The ML model retrains every Sunday at 2 AM UTC.

The pipeline collects signal/outcome pairs from the trailing 90 days, trains a gradient-boosted model, and outputs new confidence weights. Bots pick up new scores within 60 seconds.

The edge decay report also runs every Sunday at 11 PM UTC to flag strategies whose rolling hit rate dropped >10% below their backtest baseline.

Manual retrain (admin only): POST /api/ml-ops/retrain`},{q:"Can I use Aura Alpha on mobile?",a:`Yes. The iOS app is available on TestFlight (beta). Built with Capacitor 8. Features:
• Live position monitoring
• Kill switch with confirmation
• Bot status and heartbeats
• Signal feed
• Alert management
• Push notifications via ntfy

To join TestFlight, email support@auraalpha.cc with your Apple ID email. Android is in development.`},{q:"What is the founding spot discount?",a:`The founding spot discount is a permanent 25% off any paid subscription, available to early users on a limited basis. Check the pricing page for remaining spots.

Use code FOUNDER25 at checkout to lock in the permanent discount at any tier.

LAUNCH50 is a separate 50% off your first month only (not permanent).`},{q:"What happens when I hit the kill switch?",a:`1. All new signal generation halts within 100ms (Redis pub/sub propagation).
2. All bots stop submitting new orders.
3. Open positions are NOT automatically closed (intentional — rushed exits cause worse fills).
4. Kill switch indicator turns red in the top nav bar.

To reset: Control Plane > Reset Kill Switch.`},{q:"What is signal confidence?",a:`A composite 0–1 score:
• 40% — ML model probability (trained on historical signal/outcome pairs)
• 30% — Backtest Sharpe ratio of the strategy in the current regime
• 20% — Rolling 30-day live hit rate (if available)
• 10% — Volume/liquidity quality of the symbol at time of signal

Nova additionally applies a Gemini Flash overlay that can boost or reduce the score based on broader market context.

Bots require a minimum of 0.50. This threshold is configurable per-bot.`},{q:"Can I run strategies on international markets?",a:`Yes, depending on your subscription:
• Explorer / Trader: US only
• Starter: US + 3 regions
• Active: US + 7 regions
• Pro / Elite: All 14 regions

Regions: US, UK (LSE), Germany (Xetra), Japan (TSE), Hong Kong (HKEX), Canada (TSX), India (BSE), and 7 more.

Note: International markets require separate IBKR market data subscriptions.`},{q:"How do I backtest a strategy?",a:`1. Go to Backtest Dashboard.
2. Select a strategy, set the date range, and choose a universe.
3. Click Run Backtest.
4. Review: win rate (>45%), profit factor (>1.5), max drawdown (<20%), Sharpe (>1.0), trade count (>30).
5. If results look solid, click Promote to Paper.`},{q:"What is paper trading and is it realistic?",a:`Paper trading simulates trades with real-time prices but no real orders.

Realistic:
• Signal generation is identical to live mode
• Entry/exit prices use actual bid/ask mid-price at time of signal

Less realistic:
• Fills are instantaneous; real fills may queue
• No market impact (your real orders move the price; paper orders don't)
• Slippage is estimated at 0.05% per trade

For large positions (>$50K/trade), paper results are likely more optimistic than live.`},{q:"What ports does Aura Alpha use?",a:`Control Plane API: 8020 (FastAPI, all /api/ endpoints)
Frontend:          8181 (Vite dev server or nginx in production)
IBKR Gateway:      4001 (IB Gateway TWS socket)
Redis:             6379 (rate limiting + kill switch, optional)
PostgreSQL:        5432 (internal only)

In production (EC2), nginx serves the frontend on port 443 (HTTPS) and proxies /api/ to localhost:8020.`},{q:"How do I restart a crashed bot?",a:`Bots are systemd services:

  systemctl status aura-bot@alpha
  systemctl restart aura-bot@alpha
  journalctl -u aura-bot@alpha -f --since "1 hour ago"

The autorepair cron (ops/ec2_autorepair.sh) runs every 2 minutes and automatically restarts bots that stop unexpectedly.`},{q:"Is there a desktop app?",a:`Yes. The Aura Alpha Desktop app (Tauri v2, v1.5.0) provides:
• Native OS notifications (macOS, Windows)
• System tray integration
• Lower memory usage than a browser tab
• Same-machine IBKR Gateway integration (no proxy needed)
• Bundled Python for local backtest computation

Download at auraalpha.cc/download. Current version: 1.5.0.`},{q:"What is the Portfolio Brain?",a:`The Portfolio Brain is a cross-bot monitoring system that:
• Tracks combined positions across Alpha, Beta, and Crypto bots
• Detects conflicts (e.g., Alpha long XYZ while Beta short XYZ)
• Analyzes total exposure by sector, geography, and asset class
• Suggests allocation adjustments

Important: the Portfolio Brain is READ-ONLY and SUGGEST-ONLY. It never automatically rebalances or closes positions. You must act on suggestions manually.

View: GET /api/portfolio-brain/snapshot`},{q:"What is the Ensemble Score?",a:`The ensemble scorer combines signals from multiple strategies pointing to the same symbol at the same time.

If 3 different strategies all signal "buy AAPL" within a 30-minute window, the ensemble score for AAPL is higher than any single strategy's score.

High ensemble scores often indicate stronger setups because multiple independent rules agree.

View ensemble scores: GET /api/ensemble/scores (also visible in Scanner pages — sortable column).`},{q:"What is cautious mode?",a:`Cautious mode is a system-wide risk throttle that activates when:
• VIX > 30, OR
• 14-day realized volatility of SPY > 25%, OR
• Portfolio drawdown from peak > 15%

In cautious mode:
• Position sizes halved (0.5× Kelly multiplier)
• Strategies with Sharpe <1.0 are paused
• Top 10 highest-confidence signals per bot only
• Crypto bot exposure cap: 95% → 60%

Force cautious: POST /api/control/regime {"mode": "cautious"}`},{q:"Is my trading data sent to Aura Alpha's servers?",a:`Your IBKR credentials are NEVER sent to Aura Alpha's servers — they're entered directly into IB Gateway which runs locally.

Aura Alpha's servers receive:
• Aggregated position data (symbol, quantity, average cost, current value)
• Signal logs and backtest results
• Account equity snapshots (for risk calculations) — no account number or identifying info

For GDPR data requests: privacy@auraalpha.cc
For data export: Settings > Account > Export Data`},{q:"How do I cancel my subscription?",a:`Navigate to Settings > Billing and click Cancel Subscription. Access continues to the end of the current billing period. No partial refunds.

Alternatively, email support@auraalpha.cc with your account email.

Your data is retained for 90 days after cancellation. After 90 days, it is permanently deleted. Reactivating within 90 days restores all data.`}];function he(){var b,f;B("Documentation");const[r,S]=l.useState("getting-started"),[p,k]=l.useState({}),[n,u]=l.useState(""),[h,m]=l.useState(!1),g=(a,o)=>{const s=`${a}-${o}`;k(i=>({...i,[s]:!i[s]}))},y=l.useMemo(()=>{if(!n.trim())return w;const a=n.toLowerCase();return w.filter(o=>o.q.toLowerCase().includes(a)||o.a.toLowerCase().includes(a))},[n]),d=a=>{S(a),m(!1),window.scrollTo({top:0,behavior:"smooth"})};return e.jsxs("div",{className:t.page,children:[e.jsx(A,{}),e.jsxs("section",{className:t.hero,children:[e.jsx("div",{className:t.heroLogoWrap,children:e.jsx(T,{variant:"icon",height:64,glow:!0})}),e.jsx("h1",{className:t.heroTitle,children:"Documentation"}),e.jsx("p",{className:t.heroSub,children:"Everything you need to set up, configure, and trade with Aura Alpha."})]}),e.jsxs("div",{className:t.mobileNavToggle,children:[e.jsxs("button",{className:t.mobileNavBtn,onClick:()=>m(a=>!a),children:[e.jsx("span",{dangerouslySetInnerHTML:{__html:((b=c.find(a=>a.id===r))==null?void 0:b.icon)||"&#128640;"}}),(f=c.find(a=>a.id===r))==null?void 0:f.title,e.jsx("span",{className:t.mobileNavChevron,children:h?"▲":"▼"})]}),h&&e.jsxs("div",{className:t.mobileNavDropdown,children:[c.map(a=>e.jsxs("button",{onClick:()=>d(a.id),className:r===a.id?t.mobileNavItemActive:t.mobileNavItem,children:[e.jsx("span",{dangerouslySetInnerHTML:{__html:a.icon}}),a.title]},a.id)),e.jsxs("button",{onClick:()=>d("faq"),className:r==="faq"?t.mobileNavItemActive:t.mobileNavItem,children:[e.jsx("span",{children:"❓"})," FAQ"]})]})]}),e.jsxs("div",{className:t.contentWrap,children:[e.jsxs("nav",{className:t.sidebar,children:[c.map(a=>e.jsxs("button",{onClick:()=>d(a.id),className:r===a.id?t.sidebarBtnActive:t.sidebarBtn,children:[e.jsx("span",{dangerouslySetInnerHTML:{__html:a.icon}}),a.title]},a.id)),e.jsxs("button",{onClick:()=>d("faq"),className:r==="faq"?t.sidebarBtnActive:t.sidebarBtn,children:[e.jsx("span",{children:"❓"}),"FAQ"]})]}),e.jsxs("div",{className:t.contentArea,children:[r!=="faq"&&c.filter(a=>a.id===r).map(a=>e.jsxs("div",{children:[e.jsxs("h2",{className:t.sectionTitle,children:[e.jsx("span",{dangerouslySetInnerHTML:{__html:a.icon}}),a.title]}),a.items.map((o,s)=>{const i=`${a.id}-${s}`,v=p[i]!==!1;return e.jsxs("div",{className:t.docCard,children:[e.jsxs("button",{onClick:()=>g(a.id,s),className:t.docCardHeader,children:[e.jsx("span",{className:t.docCardTitle,children:o.title}),e.jsx("span",{className:v?t.docCardChevronOpen:t.docCardChevron,children:"▼"})]}),v&&e.jsx("div",{className:t.docCardBody,style:{borderTop:"1px solid rgba(255,255,255,0.03)"},children:e.jsx("pre",{className:t.docCardBodyContent,children:o.content})})]},s)})]},a.id)),r==="faq"&&e.jsxs("div",{children:[e.jsxs("h2",{className:t.sectionTitle,children:[e.jsx("span",{children:"❓"}),"Frequently Asked Questions"]}),e.jsxs("div",{className:t.faqSearchWrap,children:[e.jsx("span",{className:t.faqSearchIcon,children:"🔍"}),e.jsx("input",{className:t.faqSearch,type:"text",placeholder:"Search questions...",value:n,onChange:a=>u(a.target.value)}),n&&e.jsx("button",{className:t.faqSearchClear,onClick:()=>u(""),children:"✕"})]}),y.length===0&&e.jsxs("div",{className:t.faqNoResults,children:['No questions found for "',n,'"']}),y.map((a,o)=>{const s=`faq-${o}`,i=p[s]!==!1;return e.jsxs("div",{className:t.docCard,children:[e.jsxs("button",{onClick:()=>g("faq",o),className:t.docCardHeader,children:[e.jsx("span",{className:t.docCardTitle,children:a.q}),e.jsx("span",{className:i?t.docCardChevronOpen:t.docCardChevron,children:"▼"})]}),i&&e.jsx("div",{className:t.docCardBody,style:{borderTop:"1px solid rgba(255,255,255,0.03)"},children:e.jsx("pre",{className:t.docCardBodyContent,children:a.a})})]},o)})]})]})]}),e.jsxs("section",{className:t.ctaSection,children:[e.jsx("h2",{className:t.ctaTitle,children:"Need Help?"}),e.jsx("p",{className:t.ctaSub,children:"Can't find what you're looking for? Reach out to our team."}),e.jsx("a",{href:"mailto:support@auraalpha.cc",className:t.ctaLink,children:"Contact Support"})]}),e.jsx(C,{})]})}export{he as default};
//# sourceMappingURL=DocsPage-6AujSnMx.js.map
