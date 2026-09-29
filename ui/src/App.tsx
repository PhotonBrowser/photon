import { PhotonWebView } from "./components/PhotonWebView";

declare const Bun: { env: Record<string, string | undefined> };

export function App() {
  return (
    <div
      style={{
        width: "100%",
        height: "100%",
        padding: 4,
        backgroundColor: "#111111",
        borderRadius: 12,
      }}
    >
      <PhotonWebView
        url={Bun.env.PHOTON_URL ?? "https://example.com"}
        style={{
          width: "100%",
          height: "100%",
          borderRadius: 6,
          overflow: "hidden",
        }}
      />
    </div>
  );
}
