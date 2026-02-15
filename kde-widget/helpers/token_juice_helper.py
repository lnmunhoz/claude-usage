#!/usr/bin/env python3
"""
Token Juice KDE Plasma Widget Helper

Fetches usage data for Cursor and Claude, outputs JSON to stdout.
Called as: python3 token_juice_helper.py <cursor|claude>

Dependencies:
  pip install rookiepy requests
"""

import json
import os
import sys
from pathlib import Path

import requests
import rookiepy


# ---------------------------------------------------------------------------
# Cursor
# ---------------------------------------------------------------------------

CURSOR_DOMAINS = ["cursor.com", "cursor.sh"]
SESSION_COOKIE_NAMES = [
    "WorkosCursorSessionToken",
    "__Secure-next-auth.session-token",
    "next-auth.session-token",
]


def fetch_cursor_usage() -> dict:
    """Fetch Cursor usage via browser cookies."""
    cookies = rookiepy.load(CURSOR_DOMAINS)
    if not cookies:
        raise RuntimeError(
            "No cookies found for cursor.com. Make sure you are logged in."
        )

    # Find a session cookie
    session_found = False
    for c in cookies:
        if c.get("name") in SESSION_COOKIE_NAMES:
            session_found = True
            break

    if not session_found:
        raise RuntimeError(
            "No Cursor session cookie found. Log into cursor.com in your browser."
        )

    # Build cookie header from all cursor-domain cookies
    cookie_header = "; ".join(f"{c['name']}={c['value']}" for c in cookies)

    resp = requests.get(
        "https://cursor.com/api/usage-summary",
        headers={"Accept": "application/json", "Cookie": cookie_header},
        timeout=15,
    )

    if resp.status_code in (401, 403):
        raise RuntimeError("Not logged in. Log into cursor.com in your browser.")
    resp.raise_for_status()

    summary = resp.json()

    individual = summary.get("individualUsage") or {}
    plan = individual.get("plan") or {}
    on_demand = individual.get("onDemand") or {}

    used_cents = plan.get("used", 0)
    limit_cents = plan.get("limit", 0)
    remaining_cents = plan.get("remaining", 0)

    percent_used = (used_cents / limit_cents * 100.0) if limit_cents > 0 else 0.0

    od_used_cents = on_demand.get("used", 0)
    od_limit_cents = on_demand.get("limit")
    on_demand_percent = (
        (od_used_cents / od_limit_cents * 100.0)
        if od_limit_cents and od_limit_cents > 0
        else 0.0
    )

    return {
        "percentUsed": clamp(percent_used),
        "usedUsd": used_cents / 100.0,
        "limitUsd": limit_cents / 100.0,
        "remainingUsd": remaining_cents / 100.0,
        "onDemandPercentUsed": clamp(on_demand_percent),
        "onDemandUsedUsd": od_used_cents / 100.0,
        "onDemandLimitUsd": (od_limit_cents / 100.0) if od_limit_cents else None,
        "billingCycleEnd": summary.get("billingCycleEnd"),
        "membershipType": summary.get("membershipType"),
    }


# ---------------------------------------------------------------------------
# Claude
# ---------------------------------------------------------------------------

CLAUDE_DOMAIN = "claude.ai"


def _claude_credentials_path() -> Path | None:
    """Find the Claude OAuth credentials file."""
    config_dir_env = os.environ.get("CLAUDE_CONFIG_DIR", "")
    if config_dir_env:
        for root in config_dir_env.split(","):
            root = root.strip()
            if not root:
                continue
            candidate = Path(root) / ".credentials.json"
            if candidate.exists():
                return candidate

    home = Path.home()
    candidates = [
        home / ".claude" / ".credentials.json",
        home / ".config" / "claude" / ".credentials.json",
    ]
    for candidate in candidates:
        if candidate.exists():
            return candidate

    return None


def _load_claude_oauth_token() -> str | None:
    """Load Claude OAuth access token from credentials file."""
    path = _claude_credentials_path()
    if path is None:
        return None

    try:
        raw = json.loads(path.read_text())
    except (json.JSONDecodeError, OSError):
        return None

    # Support keychain-style { "claudeAiOauth": { "accessToken": "..." } }
    oauth_blob = raw.get("claudeAiOauth")
    if oauth_blob and isinstance(oauth_blob, dict):
        token = oauth_blob.get("accessToken") or oauth_blob.get("access_token")
    else:
        # Flat format { "accessToken": "..." }
        token = raw.get("accessToken") or raw.get("access_token")

    if token and token.startswith("sk-ant-oat"):
        return token
    return None


def _extract_window_percent(window: dict | None) -> float:
    """Extract percentage from a usage window object."""
    if window is None:
        return 0.0
    # Primary: utilization (0-100)
    if "utilization" in window and window["utilization"] is not None:
        return float(window["utilization"])
    if "percent_used" in window and window["percent_used"] is not None:
        return float(window["percent_used"])
    if "percent_left" in window and window["percent_left"] is not None:
        return 100.0 - float(window["percent_left"])
    used = window.get("used")
    limit = window.get("limit")
    if used is not None and limit is not None and limit > 0:
        return float(used) / float(limit) * 100.0
    return 0.0


def _extract_window_reset(window: dict | None) -> str | None:
    """Extract reset time from a usage window object."""
    if window is None:
        return None
    return (
        window.get("reset_at")
        or window.get("resets_at")
        or window.get("reset_time")
    )


def fetch_claude_usage_oauth() -> dict:
    """Fetch Claude usage via OAuth credentials."""
    token = _load_claude_oauth_token()
    if token is None:
        raise RuntimeError("No Claude OAuth credentials available.")

    resp = requests.get(
        "https://api.anthropic.com/api/oauth/usage",
        headers={
            "Accept": "application/json",
            "Authorization": f"Bearer {token}",
            "anthropic-beta": "oauth-2025-04-20",
        },
        timeout=15,
    )
    if not resp.ok:
        raise RuntimeError(f"Claude OAuth API returned HTTP {resp.status_code}")

    data = resp.json()

    five_hour = data.get("five_hour") or data.get("current_session")
    seven_day = data.get("seven_day") or data.get("current_week")

    session_pct = _extract_window_percent(five_hour)
    weekly_pct = _extract_window_percent(seven_day)
    session_reset = _extract_window_reset(five_hour)
    weekly_reset = _extract_window_reset(seven_day)

    extra = data.get("extra_usage")
    extra_spend = None
    extra_limit = None
    if extra and extra.get("is_enabled"):
        extra_spend = (
            extra.get("used_credits")
            or extra.get("spend")
            or extra.get("used")
            or extra.get("monthly_spend")
        )
        extra_limit = extra.get("monthly_limit") or extra.get("limit")

    # Determine plan type from credentials file
    plan_type = _get_claude_plan_type_from_creds()

    return {
        "sessionPercentUsed": clamp(session_pct),
        "weeklyPercentUsed": clamp(weekly_pct),
        "sessionReset": session_reset,
        "weeklyReset": weekly_reset,
        "planType": plan_type,
        "extraUsageSpend": extra_spend,
        "extraUsageLimit": extra_limit,
    }


def _get_claude_plan_type_from_creds() -> str | None:
    """Try to get plan type from credentials file's rateLimitTier."""
    path = _claude_credentials_path()
    if path is None:
        return None
    try:
        raw = json.loads(path.read_text())
    except (json.JSONDecodeError, OSError):
        return None

    oauth_blob = raw.get("claudeAiOauth")
    if oauth_blob and isinstance(oauth_blob, dict):
        tier = oauth_blob.get("rateLimitTier") or oauth_blob.get("rate_limit_tier")
    else:
        tier = raw.get("rateLimitTier") or raw.get("rate_limit_tier")

    if not tier:
        return None
    tier = str(tier).lower()
    if "max" in tier or "scale" in tier:
        return "max"
    if "team" in tier or "enterprise" in tier:
        return "team"
    if "pro" in tier:
        return "pro"
    return tier


def fetch_claude_usage_web() -> dict:
    """Fetch Claude usage via browser session cookies (web fallback)."""
    cookies = rookiepy.load([CLAUDE_DOMAIN])
    session_keys = [
        c["value"] for c in cookies if c.get("name") == "sessionKey" and c.get("value")
    ]
    if not session_keys:
        raise RuntimeError(
            "No claude.ai sessionKey cookie found. Log into claude.ai in your browser."
        )

    last_error = None
    for session_key in session_keys:
        cookie_header = f"sessionKey={session_key}"
        headers = {"Accept": "application/json", "Cookie": cookie_header}

        try:
            # Get organization ID
            org_resp = requests.get(
                "https://claude.ai/api/organizations", headers=headers, timeout=15
            )
            if not org_resp.ok:
                last_error = f"Organizations returned HTTP {org_resp.status_code}"
                continue

            orgs = org_resp.json()
            org_id = None
            if isinstance(orgs, list) and orgs:
                org_id = orgs[0].get("uuid") or orgs[0].get("id")
            elif isinstance(orgs, dict):
                org_id = orgs.get("uuid") or orgs.get("id")

            if not org_id:
                last_error = "Could not find org ID"
                continue

            # Fetch usage
            usage_resp = requests.get(
                f"https://claude.ai/api/organizations/{org_id}/usage",
                headers=headers,
                timeout=15,
            )
            if not usage_resp.ok:
                last_error = f"Usage returned HTTP {usage_resp.status_code}"
                continue

            usage = usage_resp.json()

            five_hour = usage.get("five_hour") or usage.get("current_session")
            seven_day = usage.get("seven_day") or usage.get("current_week")

            session_pct = _extract_window_percent(five_hour)
            weekly_pct = _extract_window_percent(seven_day)
            session_reset = _extract_window_reset(five_hour)
            weekly_reset = _extract_window_reset(seven_day)

            # Try overage
            extra_spend = None
            extra_limit = None
            try:
                overage_resp = requests.get(
                    f"https://claude.ai/api/organizations/{org_id}/overage_spend_limit",
                    headers=headers,
                    timeout=15,
                )
                if overage_resp.ok:
                    ov = overage_resp.json()
                    extra_spend = (
                        ov.get("spend") or ov.get("used") or ov.get("monthly_spend")
                    )
                    extra_limit = ov.get("limit") or ov.get("monthly_limit")
            except Exception:
                pass

            # Try plan type
            plan_type = None
            try:
                acct_resp = requests.get(
                    "https://claude.ai/api/account", headers=headers, timeout=15
                )
                if acct_resp.ok:
                    acct = acct_resp.json()
                    plan_type = (
                        acct.get("plan")
                        or acct.get("plan_type")
                        or acct.get("subscription_tier")
                    )
            except Exception:
                pass

            return {
                "sessionPercentUsed": clamp(session_pct),
                "weeklyPercentUsed": clamp(weekly_pct),
                "sessionReset": session_reset,
                "weeklyReset": weekly_reset,
                "planType": plan_type,
                "extraUsageSpend": extra_spend,
                "extraUsageLimit": extra_limit,
            }

        except requests.RequestException as e:
            last_error = str(e)
            continue

    raise RuntimeError(
        f"Claude web fallback failed: {last_error or 'all session keys rejected'}"
    )


def fetch_claude_usage() -> dict:
    """Try OAuth first, fall back to web cookies."""
    try:
        return fetch_claude_usage_oauth()
    except Exception:
        return fetch_claude_usage_web()


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def clamp(value: float, lo: float = 0.0, hi: float = 100.0) -> float:
    return max(lo, min(hi, value))


def make_response(provider: str, data: dict) -> dict:
    return {"provider": provider, "ok": True, "data": data, "error": None}


def make_error(provider: str, error: str) -> dict:
    return {"provider": provider, "ok": False, "data": None, "error": error}


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------


def main():
    if len(sys.argv) < 2 or sys.argv[1] not in ("cursor", "claude"):
        print(
            json.dumps(make_error("unknown", "Usage: token_juice_helper.py <cursor|claude>"))
        )
        sys.exit(1)

    provider = sys.argv[1]

    try:
        if provider == "cursor":
            data = fetch_cursor_usage()
        else:
            data = fetch_claude_usage()
        print(json.dumps(make_response(provider, data)))
    except Exception as e:
        print(json.dumps(make_error(provider, str(e))))
        sys.exit(1)


if __name__ == "__main__":
    main()
