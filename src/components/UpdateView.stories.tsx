import type { Meta, StoryObj } from "@storybook/react-vite";
import { useEffect } from "react";
import { UpdateView } from "./UpdateView";
import { __emitToListeners } from "../__mocks__/tauri-api-event";

const decorator = (Story: React.ComponentType) => (
  <div style={{ width: 280, height: 400, background: "#1c1c1e" }}>
    <Story />
  </div>
);

const meta: Meta<typeof UpdateView> = {
  title: "Components/UpdateView",
  component: UpdateView,
  decorators: [decorator],
};

export default meta;
type Story = StoryObj<typeof UpdateView>;

export const Loading: Story = {};

export const WithUpdateInfo: Story = {
  decorators: [
    (Story) => {
      useEffect(() => {
        // Emit update info after component mounts and registers its listener
        const timer = setTimeout(() => {
          __emitToListeners("update-info", {
            version: "1.3.0",
            body: "Bug fixes and performance improvements.\n\nNew: dark mode support for the settings panel.",
          });
        }, 100);
        return () => clearTimeout(timer);
      }, []);
      return <Story />;
    },
  ],
};
