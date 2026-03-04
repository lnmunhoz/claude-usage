import type { Preview } from "@storybook/react-vite";
import "../src/App.css";

const preview: Preview = {
  parameters: {
    backgrounds: {
      default: "tray",
      values: [{ name: "tray", value: "#1c1c1e" }],
    },
    viewport: {
      viewports: {
        tray: {
          name: "Tray Popover",
          styles: { width: "280px", height: "400px" },
        },
      },
      defaultViewport: "tray",
    },
    controls: {
      matchers: {
        color: /(background|color)$/i,
        date: /Date$/i,
      },
    },
  },
};

export default preview;
