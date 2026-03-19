import { describe, expect, it, vi, beforeEach } from "vitest";
import { render, act } from "@testing-library/react";

const mockInitialize = vi.fn();
const mockRender = vi.fn().mockResolvedValue({ svg: "<svg></svg>" });

vi.mock("mermaid", () => ({
  default: {
    initialize: mockInitialize,
    render: mockRender,
  },
}));

// Import after mock registration
const { MermaidDiagram } = await import("../MermaidDiagram");

describe("MermaidDiagram security config", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("enforces securityLevel strict even when config prop requests loose", async () => {
    await act(async () => {
      render(
        <MermaidDiagram
          id="diagram-sec-1"
          chart="graph TD\n A --> B"
          config={{ securityLevel: "loose" }}
        />,
      );
    });

    expect(mockInitialize).toHaveBeenCalled();
    const cfg = mockInitialize.mock.calls[0][0];
    expect(cfg.securityLevel).toBe("strict");
  });

  it("enforces htmlLabels false even when config prop requests true", async () => {
    await act(async () => {
      render(
        <MermaidDiagram
          id="diagram-sec-2"
          chart="graph TD\n A --> B"
          config={{ flowchart: { htmlLabels: true } }}
        />,
      );
    });

    expect(mockInitialize).toHaveBeenCalled();
    const cfg = mockInitialize.mock.calls[0][0];
    expect(cfg.flowchart?.htmlLabels).toBe(false);
  });

  it("uses securityLevel strict when no config prop is provided", async () => {
    await act(async () => {
      render(<MermaidDiagram id="diagram-sec-3" chart="graph TD\n A --> B" />);
    });

    expect(mockInitialize).toHaveBeenCalled();
    const cfg = mockInitialize.mock.calls[0][0];
    expect(cfg.securityLevel).toBe("strict");
    expect(cfg.flowchart?.htmlLabels).toBe(false);
  });
});
