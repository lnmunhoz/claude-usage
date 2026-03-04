import type { Meta, StoryObj } from "@storybook/react-vite";
import { userEvent, within } from "storybook/test";
import { SetupView } from "./SetupView";
import { __setInvokeHandler, __resetInvokeHandler } from "../__mocks__/tauri-api-core";

const decorator = (Story: React.ComponentType) => (
  <div style={{ width: 280, height: 400, background: "#1c1c1e" }}>
    <Story />
  </div>
);

const meta: Meta<typeof SetupView> = {
  title: "Components/SetupView",
  component: SetupView,
  decorators: [decorator],
  args: {
    onSaved: () => console.log("saved"),
  },
};

export default meta;
type Story = StoryObj<typeof SetupView>;

export const Default: Story = {};

export const WaitingForBrowser: Story = {
  decorators: [
    (Story) => {
      __setInvokeHandler((cmd) => {
        if (cmd === "login_oauth") {
          return new Promise(() => {}); // Never resolves
        }
      });
      return <Story />;
    },
  ],
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const button = canvas.getByRole("button", { name: /login with claude/i });
    await userEvent.click(button);
  },
};

export const ErrorState: Story = {
  decorators: [
    (Story) => {
      __setInvokeHandler((cmd) => {
        if (cmd === "login_oauth") {
          throw new Error("OAuth flow failed: connection timeout");
        }
      });
      return <Story />;
    },
  ],
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const button = canvas.getByRole("button", { name: /login with claude/i });
    await userEvent.click(button);
  },
};
