import type { StorybookConfig } from "@storybook/react-vite";
import path from "path";
import { fileURLToPath } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

const config: StorybookConfig = {
  stories: ["../src/**/*.mdx", "../src/**/*.stories.@(js|jsx|mjs|ts|tsx)"],
  addons: [
    "@storybook/addon-docs",
    "@storybook/addon-a11y",
  ],
  framework: "@storybook/react-vite",
  async viteFinal(config) {
    config.resolve = config.resolve || {};
    config.resolve.alias = {
      ...config.resolve.alias,
      "@tauri-apps/api/core": path.resolve(
        __dirname,
        "../src/__mocks__/tauri-api-core.ts"
      ),
      "@tauri-apps/api/event": path.resolve(
        __dirname,
        "../src/__mocks__/tauri-api-event.ts"
      ),
      "@tauri-apps/api/window": path.resolve(
        __dirname,
        "../src/__mocks__/tauri-api-window.ts"
      ),
      "@tauri-apps/plugin-opener": path.resolve(
        __dirname,
        "../src/__mocks__/tauri-plugin-opener.ts"
      ),
    };
    return config;
  },
};

export default config;
