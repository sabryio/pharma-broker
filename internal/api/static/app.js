// PharmaBroker Dashboard Application
class Dashboard {
  constructor() {
    this.apiBase = "/api";
    this.eventSource = null;
    this.offers = [];
    this.requests = [];
    this.matches = [];

    this.init();
  }

  async init() {
    this.bindEvents();
    this.connectSSE();
    await this.loadData();
  }

  bindEvents() {
    // Search inputs with debounce
    const offersSearch = document.getElementById("offers-search");
    const requestsSearch = document.getElementById("requests-search");

    offersSearch.addEventListener(
      "input",
      this.debounce(() => {
        this.loadOffers(offersSearch.value);
      }, 300),
    );

    requestsSearch.addEventListener(
      "input",
      this.debounce(() => {
        this.loadRequests(requestsSearch.value);
      }, 300),
    );

    // Keyboard shortcuts
    document.addEventListener("keydown", (e) => {
      if (e.key === "r" && e.ctrlKey) {
        e.preventDefault();
        this.loadData();
      }
    });
  }

  connectSSE() {
    this.eventSource = new EventSource(`${this.apiBase}/events`);

    this.eventSource.addEventListener("connected", () => {
      this.setConnectionStatus("connected");
    });

    this.eventSource.addEventListener("new_offer", (e) => {
      const data = JSON.parse(e.data);
      this.toast(`New offer: ${data.medication}`, "success");
      this.loadOffers();
      this.loadStats();
    });

    this.eventSource.addEventListener("new_request", (e) => {
      const data = JSON.parse(e.data);
      this.toast(`New request: ${data.medication}`, "success");
      this.loadRequests();
      this.loadStats();
    });

    this.eventSource.addEventListener("new_match", (e) => {
      const data = JSON.parse(e.data);
      this.toast(
        `New match suggestion (${Math.round(data.score * 100)}%)`,
        "info",
      );
      this.loadMatches();
      this.loadStats();
    });

    this.eventSource.addEventListener("match_confirmed", () => {
      this.loadMatches();
      this.loadOffers();
      this.loadRequests();
      this.loadStats();
    });

    this.eventSource.addEventListener("heartbeat", () => {
      this.setConnectionStatus("connected");
    });

    this.eventSource.onerror = () => {
      this.setConnectionStatus("disconnected");
      setTimeout(() => this.connectSSE(), 5000);
    };
  }

  setConnectionStatus(status) {
    const el = document.getElementById("connection-status");
    el.className = `connection-status ${status}`;
    el.querySelector(".status-text").textContent =
      status === "connected"
        ? "Connected"
        : status === "disconnected"
          ? "Disconnected"
          : "Connecting...";
  }

  async loadData() {
    await Promise.all([
      this.loadOffers(),
      this.loadRequests(),
      this.loadMatches(),
      this.loadStats(),
    ]);
  }

  async loadStats() {
    try {
      const res = await fetch(`${this.apiBase}/stats`);
      const { data } = await res.json();

      document.getElementById("stat-offers").textContent =
        data.active_offers || 0;
      document.getElementById("stat-requests").textContent =
        data.active_requests || 0;
      document.getElementById("stat-matches").textContent =
        data.pending_matches || 0;
      document.getElementById("stat-confirmed").textContent =
        data.confirmed_today || 0;
    } catch (err) {
      console.error("Failed to load stats:", err);
    }
  }

  async loadOffers(query = "") {
    const container = document.getElementById("offers-list");
    try {
      const url = query
        ? `${this.apiBase}/offers?q=${encodeURIComponent(query)}`
        : `${this.apiBase}/offers`;
      const res = await fetch(url);
      const { data } = await res.json();

      this.offers = data || [];
      container.innerHTML = this.offers.length
        ? this.offers.map((o) => this.renderOfferCard(o)).join("")
        : '<div class="empty-state">No active offers</div>';
    } catch (err) {
      container.innerHTML =
        '<div class="empty-state">Failed to load offers</div>';
    }
  }

  async loadRequests(query = "") {
    const container = document.getElementById("requests-list");
    try {
      const url = query
        ? `${this.apiBase}/requests?q=${encodeURIComponent(query)}`
        : `${this.apiBase}/requests`;
      const res = await fetch(url);
      const { data } = await res.json();

      this.requests = data || [];
      container.innerHTML = this.requests.length
        ? this.requests.map((r) => this.renderRequestCard(r)).join("")
        : '<div class="empty-state">No active requests</div>';
    } catch (err) {
      container.innerHTML =
        '<div class="empty-state">Failed to load requests</div>';
    }
  }

  async loadMatches() {
    const container = document.getElementById("matches-list");
    const badge = document.getElementById("matches-count");
    try {
      const res = await fetch(`${this.apiBase}/matches`);
      const { data } = await res.json();

      this.matches = data || [];
      badge.textContent = this.matches.length;
      container.innerHTML = this.matches.length
        ? this.matches.map((m) => this.renderMatchCard(m)).join("")
        : '<div class="empty-state">No pending matches</div>';
    } catch (err) {
      container.innerHTML =
        '<div class="empty-state">Failed to load matches</div>';
    }
  }

  renderOfferCard(offer) {
    const price = offer.price
      ? `${offer.price} ${offer.currency || "EGP"}`
      : "-";
    const qty = offer.quantity ? `${offer.quantity} ${offer.unit || ""}` : "";

    return `
            <div class="card" data-id="${offer.id}">
                <div class="card-header">
                    <div>
                        <div class="card-medication">${this.escape(offer.medication)}</div>
                        <div class="card-medication-raw">${this.escape(offer.medication_raw)}</div>
                    </div>
                    <div class="card-price">${price}</div>
                </div>
                <div class="card-meta">
                    ${qty ? `<span class="card-tag">${qty}</span>` : ""}
                    ${offer.expiry_date ? `<span class="card-tag">Exp: ${offer.expiry_date.substring(0, 7)}</span>` : ""}
                </div>
                <div class="card-source">
                    <span>${this.escape(offer.source_name || offer.source_phone)}</span>
                    <span>${this.timeAgo(offer.created_at)}</span>
                </div>
            </div>
        `;
  }

  renderRequestCard(request) {
    const maxPrice = request.max_price
      ? `Max: ${request.max_price} ${request.currency || "EGP"}`
      : "";
    const qty = request.quantity
      ? `${request.quantity} ${request.unit || ""}`
      : "";

    return `
            <div class="card" data-id="${request.id}">
                <div class="card-header">
                    <div>
                        <div class="card-medication">${this.escape(request.medication)}</div>
                        <div class="card-medication-raw">${this.escape(request.medication_raw)}</div>
                    </div>
                </div>
                <div class="card-meta">
                    ${qty ? `<span class="card-tag">${qty}</span>` : ""}
                    ${maxPrice ? `<span class="card-tag">${maxPrice}</span>` : ""}
                    ${request.urgent ? '<span class="card-tag urgent">URGENT</span>' : ""}
                </div>
                <div class="card-source">
                    <span>${this.escape(request.source_name || request.source_phone)}</span>
                    <span>${this.timeAgo(request.created_at)}</span>
                </div>
            </div>
        `;
  }

  renderMatchCard(match) {
    const offer = match.offer || {};
    const request = match.request || {};
    const scorePercent = Math.round(match.score * 100);

    return `
            <div class="match-card" data-id="${match.id}">
                <div class="match-header">
                    <div class="match-score">
                        <div class="score-bar">
                            <div class="score-fill" style="width: ${scorePercent}%"></div>
                        </div>
                        <span class="score-text">${scorePercent}%</span>
                    </div>
                </div>
                <div class="match-comparison">
                    <div class="match-side">
                        <div class="match-side-label">Offer</div>
                        <h4>${this.escape(offer.medication)}</h4>
                        <p>${offer.quantity || "?"} ${offer.unit || ""} @ ${offer.price || "?"} EGP</p>
                        <p style="font-size: 0.7rem; color: var(--text-muted)">${this.escape(offer.source_name || "")}</p>
                    </div>
                    <div class="match-arrow">→</div>
                    <div class="match-side">
                        <div class="match-side-label">Request</div>
                        <h4>${this.escape(request.medication)}</h4>
                        <p>${request.quantity || "?"} ${request.unit || ""} ${request.max_price ? `max ${request.max_price} EGP` : ""}</p>
                        <p style="font-size: 0.7rem; color: var(--text-muted)">${this.escape(request.source_name || "")}</p>
                    </div>
                </div>
                <div class="match-actions">
                    <button class="btn btn-reject" onclick="dashboard.rejectMatch('${match.id}')">Reject</button>
                    <button class="btn btn-confirm" onclick="dashboard.confirmMatch('${match.id}')">Confirm Match</button>
                </div>
            </div>
        `;
  }

  async confirmMatch(matchId) {
    try {
      const res = await fetch(`${this.apiBase}/matches/${matchId}/confirm`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ matched_by: "operator" }),
      });

      if (res.ok) {
        this.toast("Match confirmed! ✓", "success");
        this.loadData();
      } else {
        throw new Error("Failed to confirm");
      }
    } catch (err) {
      this.toast("Failed to confirm match", "error");
    }
  }

  async rejectMatch(matchId) {
    try {
      const res = await fetch(`${this.apiBase}/matches/${matchId}/reject`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ matched_by: "operator" }),
      });

      if (res.ok) {
        this.toast("Match rejected", "info");
        this.loadData();
      } else {
        throw new Error("Failed to reject");
      }
    } catch (err) {
      this.toast("Failed to reject match", "error");
    }
  }

  toast(message, type = "info") {
    const container = document.getElementById("toast-container");
    const toast = document.createElement("div");
    toast.className = `toast ${type}`;
    toast.textContent = message;
    container.appendChild(toast);

    setTimeout(() => toast.remove(), 4000);
  }

  timeAgo(dateStr) {
    if (!dateStr) return "";
    const date = new Date(dateStr);
    const now = new Date();
    const diff = Math.floor((now - date) / 1000);

    if (diff < 60) return "just now";
    if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
    if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
    return `${Math.floor(diff / 86400)}d ago`;
  }

  escape(str) {
    if (!str) return "";
    const div = document.createElement("div");
    div.textContent = str;
    return div.innerHTML;
  }

  debounce(fn, delay) {
    let timeout;
    return (...args) => {
      clearTimeout(timeout);
      timeout = setTimeout(() => fn.apply(this, args), delay);
    };
  }

  // Groups Modal
  openGroupsModal() {
    document.getElementById("groups-modal").style.display = "flex";
    this.loadGroups();
  }

  closeGroupsModal() {
    document.getElementById("groups-modal").style.display = "none";
  }

  async loadGroups() {
    const container = document.getElementById("groups-list");
    try {
      const res = await fetch(`${this.apiBase}/groups`);
      const { data } = await res.json();

      if (!data || data.length === 0) {
        container.innerHTML =
          '<div class="empty-state">No groups found. Click "Sync" to fetch from WhatsApp.</div>';
        return;
      }

      container.innerHTML = data.map((g) => this.renderGroupItem(g)).join("");
    } catch (err) {
      container.innerHTML =
        '<div class="empty-state">Failed to load groups</div>';
    }
  }

  async syncGroups() {
    const container = document.getElementById("groups-list");
    container.innerHTML =
      '<div class="loading">Syncing groups from WhatsApp...</div>';

    try {
      const res = await fetch(`${this.apiBase}/groups/sync`, {
        method: "POST",
      });
      const { data, error } = await res.json();

      if (error) {
        this.toast(`Sync failed: ${error}`, "error");
        container.innerHTML = `<div class="empty-state">${error}</div>`;
        return;
      }

      if (!data || data.length === 0) {
        container.innerHTML = '<div class="empty-state">No groups found</div>';
        return;
      }

      container.innerHTML = data.map((g) => this.renderGroupItem(g)).join("");
      this.toast(`Synced ${data.length} groups`, "success");
    } catch (err) {
      this.toast("Failed to sync groups", "error");
      container.innerHTML = '<div class="empty-state">Sync failed</div>';
    }
  }

  async toggleGroup(jid, monitored) {
    try {
      const res = await fetch(
        `${this.apiBase}/groups/${encodeURIComponent(jid)}`,
        {
          method: "PATCH",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ monitored }),
        },
      );

      if (res.ok) {
        this.toast(
          monitored ? "Group enabled for monitoring" : "Group disabled",
          "success",
        );
      } else {
        throw new Error("Failed to update");
      }
    } catch (err) {
      this.toast("Failed to update group", "error");
      this.loadGroups(); // Reload to reset toggle
    }
  }

  renderGroupItem(group) {
    const msgCount = group.message_count || 0;
    const lastMsg = group.last_message
      ? this.timeAgo(group.last_message)
      : "never";

    return `
            <div class="group-item">
                <div class="group-info">
                    <h4>${this.escape(group.name)}</h4>
                    <p>${msgCount} messages · Last: ${lastMsg}</p>
                </div>
                <label class="toggle-switch">
                    <input type="checkbox" ${group.monitored ? "checked" : ""} 
                           onchange="dashboard.toggleGroup('${group.jid}', this.checked)">
                    <span class="toggle-slider"></span>
                </label>
            </div>
        `;
  }
}

// Initialize
const dashboard = new Dashboard();
