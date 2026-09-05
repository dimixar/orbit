import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import {
  ComposerAttachChips,
  ComposerAttachNotice,
  ComposerDropOverlay,
} from "../ComposerAttach";

const image = {
  id: "img-1",
  name: "shot.png",
  mimeType: "image/png",
  size: 2048,
  dataUrl: "data:image/png;base64,AAAA",
};

describe("ComposerAttachChips", () => {
  it("lists staged images and removes on press", () => {
    const onRemove = vi.fn();
    render(<ComposerAttachChips images={[image]} onRemove={onRemove} />);

    expect(screen.getByRole("list", { name: "Attached images" })).toBeInTheDocument();
    expect(screen.getByText("shot.png")).toBeInTheDocument();
    expect(screen.getByText("2.0 KB")).toBeInTheDocument();

    screen.getByRole("button", { name: "Remove shot.png" }).click();
    expect(onRemove).toHaveBeenCalledWith("img-1");
  });
});

describe("ComposerAttachNotice", () => {
  it("announces a rejection", () => {
    render(
      <ComposerAttachNotice
        notice={{ tone: "danger", message: "Drop a file from this project." }}
      />,
    );
    expect(screen.getByRole("status")).toHaveTextContent(
      "Drop a file from this project.",
    );
  });
});

describe("ComposerDropOverlay", () => {
  it("stays hidden until a drag is active", () => {
    const { rerender } = render(<ComposerDropOverlay active={false} />);
    expect(screen.getByText(/Drop files from anywhere/).closest("[aria-hidden]")).toHaveAttribute(
      "aria-hidden",
      "true",
    );

    rerender(<ComposerDropOverlay active />);
    expect(screen.getByText(/Drop files from anywhere/).closest("[aria-hidden]")).toHaveAttribute(
      "aria-hidden",
      "false",
    );
  });
});
