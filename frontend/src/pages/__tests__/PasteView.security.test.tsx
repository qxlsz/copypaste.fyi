import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import {
  Link,
  MemoryRouter,
  Route,
  Routes,
  useLocation,
} from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";
import "@testing-library/jest-dom/vitest";

import { ApiError, fetchPaste } from "../../api/client";
import { PasteViewPage } from "../PasteView";

vi.mock("../../components/editor/MonacoEditor", () => ({
  MonacoEditor: ({ value }: { value: string }) => <pre>{value}</pre>,
}));

vi.mock("../../api/client", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../api/client")>();
  return { ...actual, fetchPaste: vi.fn() };
});

const mockedFetchPaste = vi.mocked(fetchPaste);

const LocationProbe = () => {
  const location = useLocation();
  return (
    <output data-testid="location">
      {location.pathname}
      {location.search}
      {location.hash}
    </output>
  );
};

beforeEach(() => {
  mockedFetchPaste.mockReset();
  mockedFetchPaste.mockResolvedValue({
    id: "paste-id",
    format: "plain_text",
    content: "decrypted content",
    createdAt: 1,
    expiresAt: null,
    burnAfterReading: false,
    encryption: { algorithm: "aes256_gcm", requiresKey: true },
    torAccessOnly: false,
  });
});

describe("PasteView sensitive query lifecycle", () => {
  it("immediately migrates a legacy query key into the fragment", async () => {
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });

    render(
      <QueryClientProvider client={queryClient}>
        <MemoryRouter
          initialEntries={[
            "/p/paste-id?view=compact&key=legacy%20secret#panel=details",
          ]}
        >
          <LocationProbe />
          <Routes>
            <Route path="/p/:id" element={<PasteViewPage />} />
          </Routes>
        </MemoryRouter>
      </QueryClientProvider>,
    );

    await waitFor(() =>
      expect(screen.getByTestId("location")).toHaveTextContent(
        "/p/paste-id?view=compact#panel=details&key=legacy+secret",
      ),
    );
    expect(await screen.findByText("decrypted content")).toBeInTheDocument();
    expect(mockedFetchPaste).toHaveBeenCalledTimes(1);
    expect(mockedFetchPaste).toHaveBeenCalledWith(
      "paste-id",
      "legacy secret",
    );
  });

  it("keeps the raw key out of the query key and clears cached content", async () => {
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    const rawKey = "raw-key-must-not-enter-query-cache";
    const view = render(
      <QueryClientProvider client={queryClient}>
        <MemoryRouter initialEntries={[`/p/paste-id#key=${rawKey}`]}>
          <Routes>
            <Route path="/p/:id" element={<PasteViewPage />} />
          </Routes>
        </MemoryRouter>
      </QueryClientProvider>,
    );

    expect(await screen.findByText("decrypted content")).toBeInTheDocument();
    expect(mockedFetchPaste).toHaveBeenCalledTimes(1);
    expect(mockedFetchPaste).toHaveBeenCalledWith("paste-id", rawKey);

    const serializedQueryKeys = JSON.stringify(
      queryClient
        .getQueryCache()
        .getAll()
        .map((query) => query.queryKey),
    );
    expect(serializedQueryKeys).not.toContain(rawKey);

    window.dispatchEvent(new Event("focus"));
    await waitFor(() => expect(mockedFetchPaste).toHaveBeenCalledTimes(1));

    view.unmount();
    expect(
      queryClient.getQueryCache().findAll({ queryKey: ["paste"] }),
    ).toHaveLength(0);
  });

  it("clears an entered key when the route changes without a key", async () => {
    mockedFetchPaste.mockRejectedValue(
      new ApiError("Invalid encryption key", 403, "invalid_key"),
    );
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });

    render(
      <QueryClientProvider client={queryClient}>
        <MemoryRouter initialEntries={["/p/first#key=first-paste-secret"]}>
          <Link to="/p/second">Next paste</Link>
          <Routes>
            <Route path="/p/:id" element={<PasteViewPage />} />
          </Routes>
        </MemoryRouter>
      </QueryClientProvider>,
    );

    expect(await screen.findByLabelText("encryption key")).toHaveValue(
      "first-paste-secret",
    );
    fireEvent.click(screen.getByRole("link", { name: "Next paste" }));

    await waitFor(() =>
      expect(screen.getByLabelText("encryption key")).toHaveValue(""),
    );
    expect(mockedFetchPaste).toHaveBeenLastCalledWith("second", undefined);
  });

  it("does not mislabel an attestation challenge as an encryption key", async () => {
    mockedFetchPaste.mockRejectedValue(
      new ApiError(
        "This paste requires an attestation code",
        401,
        "attestation_required",
      ),
    );
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });

    render(
      <QueryClientProvider client={queryClient}>
        <MemoryRouter initialEntries={["/p/paste-id"]}>
          <Routes>
            <Route path="/p/:id" element={<PasteViewPage />} />
          </Routes>
        </MemoryRouter>
      </QueryClientProvider>,
    );

    expect(
      await screen.findByText("Additional verification required"),
    ).toBeInTheDocument();
    expect(screen.queryByLabelText("encryption key")).not.toBeInTheDocument();
  });

  it("hides cached content once expiresAt has passed", async () => {
    mockedFetchPaste.mockResolvedValue({
      id: "paste-id",
      format: "plain_text",
      content: "should not remain visible",
      createdAt: 1,
      expiresAt: 1,
      burnAfterReading: false,
      encryption: { algorithm: "none", requiresKey: false },
      torAccessOnly: false,
    });
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });

    render(
      <QueryClientProvider client={queryClient}>
        <MemoryRouter initialEntries={["/p/paste-id"]}>
          <Routes>
            <Route path="/p/:id" element={<PasteViewPage />} />
          </Routes>
        </MemoryRouter>
      </QueryClientProvider>,
    );

    expect(await screen.findByLabelText("Paste not found")).toBeInTheDocument();
    expect(screen.queryByText("should not remain visible")).not.toBeInTheDocument();
  });
});
