export function getCurrentWindow() {
  return {
    label: "main",
    emit: async (_event: string, _payload?: unknown) => {},
    listen: async (_event: string, _handler: unknown) => () => {},
  };
}
