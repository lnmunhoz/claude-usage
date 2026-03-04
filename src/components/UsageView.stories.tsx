import type { Meta, StoryObj } from "@storybook/react-vite";
import { UsageView } from "./UsageView";
import type { ClaudeUsageData } from "../types";

const decorator = (Story: React.ComponentType) => (
  <div style={{ width: 280, height: 400, background: "#1c1c1e" }}>
    <Story />
  </div>
);

const baseData: ClaudeUsageData = {
  sessionPercentUsed: 35,
  weeklyPercentUsed: 20,
  sessionReset: new Date(Date.now() + 3 * 60 * 60 * 1000).toISOString(),
  weeklyReset: new Date(Date.now() + 3 * 24 * 60 * 60 * 1000).toISOString(),
  planType: "Pro",
  extraUsageSpend: null,
  extraUsageLimit: null,
};

const meta: Meta<typeof UsageView> = {
  title: "Components/UsageView",
  component: UsageView,
  decorators: [decorator],
  args: {
    data: baseData,
    displayMode: "remaining",
    onDisconnect: () => console.log("disconnect"),
  },
};

export default meta;
type Story = StoryObj<typeof UsageView>;

export const Default: Story = {};

export const LowUsage: Story = {
  args: {
    data: {
      ...baseData,
      sessionPercentUsed: 10,
      weeklyPercentUsed: 5,
    },
  },
};

export const HighUsage: Story = {
  args: {
    data: {
      ...baseData,
      sessionPercentUsed: 80,
      weeklyPercentUsed: 75,
    },
  },
};

export const MaxedOut: Story = {
  args: {
    data: {
      ...baseData,
      sessionPercentUsed: 100,
      weeklyPercentUsed: 95,
      sessionReset: new Date(Date.now() + 30 * 60 * 1000).toISOString(),
    },
  },
};

export const WithExtraUsage: Story = {
  args: {
    data: {
      ...baseData,
      extraUsageSpend: 1250,
      extraUsageLimit: 10000,
    },
  },
};

export const UsedDisplayMode: Story = {
  args: {
    displayMode: "usage",
  },
};

export const TeamPlan: Story = {
  args: {
    data: {
      ...baseData,
      planType: "Team",
      sessionPercentUsed: 50,
      weeklyPercentUsed: 40,
    },
  },
};
