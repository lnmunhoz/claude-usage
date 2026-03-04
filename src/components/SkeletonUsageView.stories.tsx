import type { Meta, StoryObj } from "@storybook/react-vite";
import { SkeletonUsageView } from "./SkeletonUsageView";

const meta: Meta<typeof SkeletonUsageView> = {
  title: "Components/SkeletonUsageView",
  component: SkeletonUsageView,
  decorators: [
    (Story) => (
      <div style={{ width: 280, height: 400, background: "#1c1c1e" }}>
        <Story />
      </div>
    ),
  ],
};

export default meta;
type Story = StoryObj<typeof SkeletonUsageView>;

export const Default: Story = {};
