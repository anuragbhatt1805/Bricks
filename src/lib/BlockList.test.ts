import { render } from "@testing-library/svelte";
import { describe, expect, it } from "vitest";
import BlockList from "./BlockList.svelte";

describe("BlockList", () => {
  it("virtualizes ten thousand blocks", () => {
    const blocks = Array.from({ length: 10_000 }, (_, index) => ({
      id: String(index),
      command: `echo ${index}`,
      cwd: "/tmp",
      exit_code: 0,
      duration_ms: 1,
      output: "ok",
    }));
    const { container } = render(BlockList, { blocks });
    expect(container.querySelectorAll(".block").length).toBeLessThan(50);
  });
});
