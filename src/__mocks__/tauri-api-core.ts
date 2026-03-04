type InvokeHandler = (cmd: string, args?: Record<string, unknown>) => unknown;

const defaultHandler: InvokeHandler = (cmd) => {
  switch (cmd) {
    case "has_token":
      return true;
    case "get_settings":
      return { displayMode: "remaining", pollIntervalSeconds: 60 };
    case "fetch_claude_usage":
      return {
        sessionPercentUsed: 35,
        weeklyPercentUsed: 20,
        sessionReset: new Date(Date.now() + 3 * 60 * 60 * 1000).toISOString(),
        weeklyReset: new Date(Date.now() + 3 * 24 * 60 * 60 * 1000).toISOString(),
        planType: "Pro",
        extraUsageSpend: null,
        extraUsageLimit: null,
      };
    case "save_poll_interval":
    case "clear_token":
    case "login_oauth":
    case "update_tray_title":
      return undefined;
    default:
      console.warn(`[mock] unhandled invoke: ${cmd}`);
      return undefined;
  }
};

let handler: InvokeHandler = defaultHandler;

export function __setInvokeHandler(h: InvokeHandler) {
  handler = h;
}

export function __resetInvokeHandler() {
  handler = defaultHandler;
}

export async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  return handler(cmd, args) as T;
}
