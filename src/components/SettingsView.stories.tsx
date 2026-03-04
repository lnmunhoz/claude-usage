import type { Meta, StoryObj } from "@storybook/react-vite";
import { SettingsView } from "./SettingsView";

const decorator = (Story: React.ComponentType) => (
  <div style={{ width: 280, height: 400, background: "#1c1c1e" }}>
    <Story />
  </div>
);

const meta: Meta<typeof SettingsView> = {
  title: "Components/SettingsView",
  component: SettingsView,
  decorators: [decorator],
  args: {
    onBack: () => console.log("back"),
  },
};

export default meta;
type Story = StoryObj<typeof SettingsView>;

export const Default: Story = {};
